//! Reading and writing the history files under the advisory lock.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::declared::{create_private_dir_all, open_private_lock, write_private_atomic};
use crate::model::{CheckoutRecord, HistoryKind, HistorySnapshot, MuxRecord, PrRecord, RepoRecord};

use super::merge::{self, MergeOutcome, Observations};

/// Schema version of every history file this binary writes.
pub const HISTORY_SCHEMA_VERSION: u32 = 1;

/// One kind's file on disk. Unknown top-level fields round-trip.
#[derive(Debug, Serialize, Deserialize)]
struct KindFile<R> {
    schema_version: u32,
    #[serde(default)]
    records: Vec<R>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

/// What a read found. Kinds whose file couldn't be read, or that a
/// newer binary wrote, are left out of `writable`, so this binary
/// never clobbers them.
#[derive(Clone, Debug, Default)]
pub struct HistoryLoad {
    pub history: HistorySnapshot,
    /// Human-readable read problems, one per file.
    pub problems: Vec<String>,
    writable: BTreeSet<HistoryKind>,
    extras: BTreeMap<HistoryKind, BTreeMap<String, Value>>,
}

impl HistoryLoad {
    pub fn is_writable(&self, kind: HistoryKind) -> bool {
        self.writable.contains(&kind)
    }
}

/// The history directory. Construct one only where history may be
/// written or read: `--no-cache`, fixture and snapshot modes, the dev
/// scenario builder, and runs without a state home have none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryStore {
    dir: PathBuf,
}

impl HistoryStore {
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `$XDG_STATE_HOME/conspectus/history`, else
    /// `$HOME/.local/state/conspectus/history`, else none.
    pub fn from_env() -> Option<Self> {
        let state = env_path("XDG_STATE_HOME")
            .or_else(|| env_path("HOME").map(|home| home.join(".local").join("state")))?;
        Some(Self::at(state.join("conspectus").join("history")))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn path(&self, kind: HistoryKind) -> PathBuf {
        self.dir.join(kind.file_name())
    }

    /// Read every file without the lock. Writes are atomic renames, so
    /// a reader sees either the old file or the new one. A missing
    /// directory is empty history.
    pub fn load(&self) -> HistoryLoad {
        let mut load = HistoryLoad::default();
        for kind in HistoryKind::ALL {
            match kind {
                HistoryKind::Mux => {
                    load.history.muxes = self.read_kind::<MuxRecord>(kind, &mut load);
                }
                HistoryKind::Checkout => {
                    load.history.checkouts = self.read_kind::<CheckoutRecord>(kind, &mut load);
                }
                HistoryKind::Repo => {
                    load.history.repos = self.read_kind::<RepoRecord>(kind, &mut load);
                }
                HistoryKind::Pr => {
                    load.history.prs = self.read_kind::<PrRecord>(kind, &mut load);
                }
            }
        }
        load
    }

    fn read_kind<R: DeserializeOwned>(&self, kind: HistoryKind, load: &mut HistoryLoad) -> Vec<R> {
        let path = self.path(kind);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                load.writable.insert(kind);
                return Vec::new();
            }
            Err(err) => {
                load.problems.push(format!("{}: {err}", path.display()));
                return Vec::new();
            }
        };
        let header: Result<KindFile<Value>, _> = serde_json::from_str(&text);
        let header = match header {
            Ok(header) => header,
            Err(err) => {
                load.problems
                    .push(format!("{}: not a history file: {err}", path.display()));
                return Vec::new();
            }
        };
        if header.schema_version > HISTORY_SCHEMA_VERSION {
            load.problems.push(format!(
                "{}: schema version {} is newer than this binary's {HISTORY_SCHEMA_VERSION}; left untouched",
                path.display(),
                header.schema_version
            ));
            return Vec::new();
        }
        let mut records = Vec::with_capacity(header.records.len());
        for (index, raw) in header.records.into_iter().enumerate() {
            match serde_json::from_value::<R>(raw) {
                Ok(record) => records.push(record),
                Err(err) => {
                    // One bad record makes the whole file read-only, so
                    // a rewrite can't drop it.
                    load.problems.push(format!(
                        "{}: record {index} is malformed ({err}); the file is left untouched",
                        path.display()
                    ));
                    return Vec::new();
                }
            }
        }
        load.writable.insert(kind);
        load.extras.insert(kind, header.extra);
        records
    }

    /// Merge `observations` into the store under the lock and return
    /// the merged history with what changed.
    pub fn record(&self, observations: &Observations) -> io::Result<(HistoryLoad, MergeOutcome)> {
        self.update(|history| merge::merge(history, observations))
    }

    /// Take the lock, re-read, apply `change`, and write each changed,
    /// writable kind atomically. Returns the merged state.
    pub fn update<F>(&self, change: F) -> io::Result<(HistoryLoad, MergeOutcome)>
    where
        F: FnOnce(&mut HistorySnapshot) -> MergeOutcome,
    {
        create_private_dir_all(&self.dir)?;
        let lock = open_private_lock(&self.dir.join("history.lock"))?;
        lock.lock()?;
        let result = self.update_locked(change);
        drop(lock);
        result
    }

    fn update_locked<F>(&self, change: F) -> io::Result<(HistoryLoad, MergeOutcome)>
    where
        F: FnOnce(&mut HistorySnapshot) -> MergeOutcome,
    {
        let mut load = self.load();
        let outcome = change(&mut load.history);
        for kind in &outcome.changed {
            if !load.is_writable(*kind) {
                continue;
            }
            let extra = load.extras.get(kind).cloned().unwrap_or_default();
            let text = match kind {
                HistoryKind::Mux => render(&load.history.muxes, extra),
                HistoryKind::Checkout => render(&load.history.checkouts, extra),
                HistoryKind::Repo => render(&load.history.repos, extra),
                HistoryKind::Pr => render(&load.history.prs, extra),
            }?;
            write_private_atomic(&self.path(*kind), &text)?;
        }
        Ok((load, outcome))
    }
}

fn render<R: Serialize>(records: &[R], extra: BTreeMap<String, Value>) -> io::Result<String> {
    #[derive(Serialize)]
    struct Out<'a, R> {
        schema_version: u32,
        records: &'a [R],
        #[serde(flatten)]
        extra: BTreeMap<String, Value>,
    }
    let file = Out {
        schema_version: HISTORY_SCHEMA_VERSION,
        records,
        extra,
    };
    let mut text = serde_json::to_string_pretty(&file).map_err(io::Error::other)?;
    text.push('\n');
    Ok(text)
}

/// Hold the lock for the duration of `f`. Used by tests that race a
/// writer against another.
#[cfg(test)]
pub(crate) fn with_lock<T>(dir: &Path, f: impl FnOnce() -> T) -> io::Result<T> {
    fs::create_dir_all(dir)?;
    let lock: fs::File = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("history.lock"))?;
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
