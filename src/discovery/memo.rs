//! Small memoization helpers shared by the discovery adapters.
//!
//! Discovery re-runs on every refresh, so adapters cache work whose
//! inputs haven't changed: a whole fragment for a short TTL
//! ([`TtlCache`]), or a per-file result that stays valid while the
//! file's modification time and length are unchanged ([`StampedMap`]
//! with a [`FileStamp`]). Locks recover from poisoning: the caches
//! are best-effort, so a panic elsewhere shouldn't make discovery
//! fail.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant, SystemTime};

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
    pub(crate) const fn new(ttl: Duration) -> Self {
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

    pub(crate) fn clear(&self) {
        *lock(&self.slot) = None;
    }
}

/// Per-path cache whose entries stay valid while a stamp matches.
pub(crate) struct StampedMap<S, V> {
    entries: Mutex<Option<HashMap<PathBuf, (S, V)>>>,
}

impl<S: PartialEq, V: Clone> StampedMap<S, V> {
    pub(crate) const fn new() -> Self {
        Self {
            entries: Mutex::new(None),
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
        let (stamp, value) = entries.as_ref()?.get(path)?;
        is_current(stamp, value).then(|| value.clone())
    }

    pub(crate) fn insert(&self, path: PathBuf, stamp: S, value: V) {
        lock(&self.entries)
            .get_or_insert_with(HashMap::new)
            .insert(path, (stamp, value));
    }

    #[cfg(test)]
    pub(crate) fn clear(&self) {
        *lock(&self.entries) = None;
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
