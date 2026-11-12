//! Reading and writing `live-state.json` under the advisory lock.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::declared::{create_private_dir_all, open_private_lock, write_private_atomic};

use super::event::{LiveEvent, LiveSnapshot};
use super::file::{LiveFile, Parsed};
use super::process::process_liveness;
use super::reducer::{Reducer, SharedReducer};

/// The store file's name, in the state directory.
pub const LIVE_STATE_FILE: &str = "live-state.json";

/// The lock file writers hold while they change the store.
pub const LIVE_STATE_LOCK: &str = "live-state.lock";

/// How long a writer waits for the lock before it drops its event.
pub const LOCK_WAIT: Duration = Duration::from_millis(200);

/// How often a waiting writer tries the lock again.
const LOCK_POLL: Duration = Duration::from_millis(2);

/// What a write did. Only a failure to read or write the file is an
/// error; a busy lock is not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Applied {
    /// The file was rewritten at revision `rev`.
    Written { rev: u64 },
    /// Nothing changed, so nothing was written: a snapshot older than
    /// the entry, or an event the entry already reflects or ignores.
    Unchanged,
    /// The lock stayed busy past [`LOCK_WAIT`], and the input was
    /// dropped.
    Dropped,
    /// An id in the input did not look like an id, and the input was
    /// dropped.
    Rejected,
    /// A newer version wrote the file, and it was left alone.
    NewerSchema,
}

/// The live-state store in a state directory. Construct one only where
/// live state may be read or written; runs without a state home have
/// none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveStore {
    dir: PathBuf,
    lock_wait: Duration,
}

/// What the file held when a writer read it.
enum Load {
    File(LiveFile),
    Newer,
}

impl LiveStore {
    /// The store whose files live in `dir`.
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            lock_wait: LOCK_WAIT,
        }
    }

    /// `$XDG_STATE_HOME/conspectus`, else `$HOME/.local/state/conspectus`,
    /// else none.
    pub fn from_env() -> Option<Self> {
        let state = env_path("XDG_STATE_HOME")
            .or_else(|| env_path("HOME").map(|home| home.join(".local").join("state")))?;
        Some(Self::at(state.join("conspectus")))
    }

    /// The same store with another lock wait.
    pub fn with_lock_wait(mut self, lock_wait: Duration) -> Self {
        self.lock_wait = lock_wait;
        self
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The store file.
    pub fn path(&self) -> PathBuf {
        self.dir.join(LIVE_STATE_FILE)
    }

    fn lock_path(&self) -> PathBuf {
        self.dir.join(LIVE_STATE_LOCK)
    }

    /// The store as it is now, without the lock: writes are atomic
    /// renames, so this is the file before or after any write. A file
    /// that is missing, unreadable, corrupt, or from a newer version
    /// reads as no live state. Never writes.
    pub fn read(&self) -> LiveFile {
        let Ok(text) = fs::read_to_string(self.path()) else {
            return LiveFile::default();
        };
        match LiveFile::parse(&text) {
            Parsed::File(file) => file,
            Parsed::Corrupt | Parsed::Newer => LiveFile::default(),
        }
    }

    /// Fold `event` into its session's entry with the shared reducer.
    pub fn apply_event(&self, event: &LiveEvent) -> io::Result<Applied> {
        self.apply_events(&SharedReducer, std::slice::from_ref(event))
    }

    /// Fold `events` in order with `reducer`, under one lock and as one
    /// write, for a payload that says several things at once. When an id
    /// in any of them does not look like one, nothing is applied.
    pub fn apply_events(&self, reducer: &dyn Reducer, events: &[LiveEvent]) -> io::Result<Applied> {
        let Some(events) = events
            .iter()
            .map(LiveEvent::sanitized)
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(Applied::Rejected);
        };
        self.write_with(|file, now_ms| {
            file.apply_events(reducer, &events, now_ms, &process_liveness)
        })
    }

    /// Replace the snapshot's entry when the snapshot is newer.
    pub fn apply_snapshot(&self, snapshot: &LiveSnapshot) -> io::Result<Applied> {
        let Some(snapshot) = snapshot.sanitized() else {
            return Ok(Applied::Rejected);
        };
        self.write_with(|file, now_ms| file.apply_snapshot(&snapshot, now_ms, &process_liveness))
    }

    /// Take the lock, re-read the file, apply `change` at the store's
    /// clock, and write the file atomically when it changed anything.
    fn write_with(&self, change: impl FnOnce(&mut LiveFile, i64) -> bool) -> io::Result<Applied> {
        create_private_dir_all(&self.dir)?;
        let lock = open_private_lock(&self.lock_path())?;
        if !acquire(&lock, self.lock_wait)? {
            return Ok(Applied::Dropped);
        }
        let result = self.write_locked(change);
        drop(lock);
        result
    }

    fn write_locked(&self, change: impl FnOnce(&mut LiveFile, i64) -> bool) -> io::Result<Applied> {
        let mut file = match self.load()? {
            Load::File(file) => file,
            Load::Newer => return Ok(Applied::NewerSchema),
        };
        if !change(&mut file, now_ms()) {
            return Ok(Applied::Unchanged);
        }
        let text = file.render().map_err(io::Error::other)?;
        write_private_atomic(&self.path(), &text)?;
        Ok(Applied::Written { rev: file.rev })
    }

    /// The file for a writer. A missing or corrupt file starts empty, so
    /// a damaged store heals on the next write; an unreadable one is an
    /// error, and a newer one is left alone.
    fn load(&self) -> io::Result<Load> {
        let text = match fs::read_to_string(self.path()) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(Load::File(LiveFile::default()));
            }
            Err(err) => return Err(err),
        };
        Ok(match LiveFile::parse(&text) {
            Parsed::File(file) => Load::File(file),
            Parsed::Corrupt => Load::File(LiveFile::default()),
            Parsed::Newer => Load::Newer,
        })
    }
}

/// Try the lock until it is ours or `wait` has passed. `Ok(false)` means
/// another writer kept it.
fn acquire(lock: &fs::File, wait: Duration) -> io::Result<bool> {
    let deadline = Instant::now() + wait;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(true),
            Err(fs::TryLockError::WouldBlock) => {}
            Err(fs::TryLockError::Error(err)) => return Err(err),
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(false);
        }
        thread::sleep(LOCK_POLL.min(deadline - now));
    }
}

/// The store's clock: Unix epoch milliseconds.
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// Hold the lock for the duration of `f`, as another writer would. Used
/// by tests that race a writer against it.
#[cfg(test)]
pub(crate) fn with_lock<T>(dir: &Path, f: impl FnOnce() -> T) -> io::Result<T> {
    create_private_dir_all(dir)?;
    let lock = open_private_lock(&dir.join(LIVE_STATE_LOCK))?;
    lock.lock()?;
    Ok(f())
}

fn env_path(key: &str) -> Option<PathBuf> {
    // A relative base directory is ignored, as the XDG specification says.
    match std::env::var_os(key).map(PathBuf::from) {
        Some(path) if path.is_absolute() => Some(path),
        _ => None,
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
