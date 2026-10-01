//! Memoized discovery work, kept between discovery runs.
//!
//! Discovery re-runs on every refresh, so adapters cache work whose
//! inputs haven't changed: a whole fragment for a short TTL
//! ([`TtlCache`]), or a per-file result that stays valid while the
//! file's modification time and length are unchanged ([`StampedMap`]
//! with a [`FileStamp`]). All of it lives in one [`DiscoveryCaches`]
//! value owned by whoever runs discovery repeatedly (ADR 0098). Locks
//! recover from poisoning: the caches are best-effort, so a panic
//! elsewhere shouldn't make discovery fail.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

use super::harness::{claude_code, codex, opencode};
use super::{GraphFragment, codex_log, forge, git, tmux, zellij};

/// Results that discovery reuses across runs while their inputs are
/// unchanged. The daemon and the TUI keep one for their lifetime and
/// pass it to every run through
/// [`LocalDiscoveryConfig::with_caches`](super::LocalDiscoveryConfig::with_caches);
/// a one-shot command starts with an empty one.
pub struct DiscoveryCaches {
    pub(crate) git_probes: StampedMap<git::GitStateFingerprint, git::CachedProbe>,
    pub(crate) tmux: TtlCache<(), GraphFragment>,
    pub(crate) zellij: TtlCache<(), GraphFragment>,
    /// The last forge fragment, keyed by the scan roots it was built
    /// for, so changing `--scan-root` misses and triggers a fresh `gh` run.
    pub(crate) forge: TtlCache<Vec<PathBuf>, GraphFragment>,
    pub(crate) codex_rollouts: StampedMap<FileStamp, codex::RolloutScan>,
    pub(crate) claude_sessions: StampedMap<FileStamp, claude_code::DiscoveredSession>,
    pub(crate) opencode_sessions: StampedMap<FileStamp, Vec<opencode::SessionInfo>>,
    pub(crate) codex_log: codex_log::QueryCache,
    /// Fingerprint of the last mux/harness slice, used to skip the
    /// process-tree walk when that slice hasn't changed.
    pub(crate) process_tree_fingerprint: Mutex<Option<u64>>,
}

impl Default for DiscoveryCaches {
    fn default() -> Self {
        Self {
            git_probes: StampedMap::new(),
            tmux: TtlCache::new(tmux::TMUX_CACHE_TTL),
            zellij: TtlCache::new(zellij::ZELLIJ_CACHE_TTL),
            forge: TtlCache::new(forge::FORGE_CACHE_TTL),
            codex_rollouts: StampedMap::new(),
            claude_sessions: StampedMap::new(),
            opencode_sessions: StampedMap::new(),
            codex_log: codex_log::QueryCache::default(),
            process_tree_fingerprint: Mutex::new(None),
        }
    }
}

impl fmt::Debug for DiscoveryCaches {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DiscoveryCaches").finish_non_exhaustive()
    }
}

impl DiscoveryCaches {
    /// The last mux/harness slice fingerprint, replaced by `current`
    /// when one was observed.
    pub(crate) fn swap_process_tree_fingerprint(&self, current: Option<u64>) -> Option<u64> {
        let mut last = lock(&self.process_tree_fingerprint);
        let previous = *last;
        if current.is_some() {
            *last = current;
        }
        previous
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A file's modification time and length, used to tell whether a
/// cached result derived from the file is still current.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileStamp {
    pub(crate) modified: SystemTime,
    pub(crate) len: u64,
}

impl FileStamp {
    /// `None` when the file can't be stat'ed or the platform doesn't
    /// report modification times.
    pub(crate) fn of(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        Some(Self {
            modified: meta.modified().ok()?,
            len: meta.len(),
        })
    }

    /// Modification time as whole seconds since the Unix epoch.
    pub(crate) fn modified_epoch(&self) -> Option<i64> {
        let secs = self
            .modified
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        i64::try_from(secs).ok()
    }
}

/// Modification time of `path`, if it can be read.
pub(crate) fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

/// One value, reused while it is younger than the TTL and was stored
/// under an equal key (for example the scan roots it was computed for).
pub(crate) struct TtlCache<K, V> {
    ttl: Duration,
    slot: Mutex<Option<(K, Instant, V)>>,
}

impl<K: PartialEq, V: Clone> TtlCache<K, V> {
    pub(crate) fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            slot: Mutex::new(None),
        }
    }

    pub(crate) fn get(&self, key: &K) -> Option<V> {
        let slot = lock(&self.slot);
        let (cached_key, stored_at, value) = slot.as_ref()?;
        (cached_key == key && stored_at.elapsed() < self.ttl).then(|| value.clone())
    }

    pub(crate) fn set(&self, key: K, value: V) {
        *lock(&self.slot) = Some((key, Instant::now(), value));
    }
}

/// Per-path cache whose entries stay valid while a stamp matches.
pub(crate) struct StampedMap<S, V> {
    entries: Mutex<HashMap<PathBuf, (S, V)>>,
    /// Number of inserts, which is the number of misses that were
    /// recomputed. Tests read it to check that a hit skipped the work.
    #[cfg(test)]
    inserts: std::sync::atomic::AtomicUsize,
}

impl<S: PartialEq, V: Clone> StampedMap<S, V> {
    pub(crate) fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            #[cfg(test)]
            inserts: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// The cached value for `path` if it was stored with `stamp`.
    pub(crate) fn get(&self, path: &Path, stamp: &S) -> Option<V> {
        self.get_if(path, |cached, _| cached == stamp)
    }

    /// The cached value for `path` if `is_current` accepts the stored
    /// stamp and value. Use this when the current stamp can only be
    /// computed from the cached value.
    pub(crate) fn get_if(&self, path: &Path, is_current: impl FnOnce(&S, &V) -> bool) -> Option<V> {
        let entries = lock(&self.entries);
        let (stamp, value) = entries.get(path)?;
        is_current(stamp, value).then(|| value.clone())
    }

    pub(crate) fn insert(&self, path: PathBuf, stamp: S, value: V) {
        #[cfg(test)]
        self.inserts
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        lock(&self.entries).insert(path, (stamp, value));
    }

    /// Inserts since the last call.
    #[cfg(test)]
    pub(crate) fn take_inserts(&self) -> usize {
        self.inserts.swap(0, std::sync::atomic::Ordering::Relaxed)
    }
}

impl<S: PartialEq, V: Clone> Default for StampedMap<S, V> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "memo_tests.rs"]
mod tests;
