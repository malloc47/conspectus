//! The TUI's live-state overlay (ADR 0205): the live-state store bound
//! again to the snapshot the TUI holds, so a hook event reaches the rows
//! in about a second instead of at the next snapshot, with or without a
//! daemon.
//!
//! A snapshot carries the `node_live` table its discovery bound, which is
//! as old as the snapshot. The overlay keeps a table of its own, bound
//! with the same [`bind_live`], and the rows read it in place of the
//! snapshot's ([`LiveOverlay::table`]). It is cheap when nothing
//! happens: each tick costs one stat of the store file, and the file is
//! read and bound (one pid check per bound entry) only when the stat
//! shows a rewrite and at least [`REREAD_INTERVAL`] has passed since the
//! last read. Every new snapshot is bound again before its rows are
//! built, so the table is never keyed to the nodes of the snapshot it
//! replaced. A missing, unreadable, or corrupt store is an empty table,
//! reported nowhere.
//!
//! An overlay that is off ([`LiveOverlay::off`], the default) leaves the
//! rows reading the snapshot's own table, as `--snapshot` and the fixture
//! and scenario TUIs do: what they draw comes from the snapshot alone.
//!
//! It reads the store and `/proc` only; it writes nothing and starts no
//! process.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::discovery::live_state::bind_live;
use crate::live_store::{LiveFile, LiveStore, PidProbe, ProcProbe};
use crate::model::live::LiveBinding;
use crate::model::{GraphSnapshot, NodeId};
use crate::tui::rows::LiveCell;
use crate::tui::rows::live::cells_at;

/// The least time between two reads of the store. A store rewritten
/// sooner is read once this has passed.
pub(crate) const REREAD_INTERVAL: Duration = Duration::from_secs(1);

/// How often the overlay checks whether time alone moved a state the
/// rows show (a working agent going quiet, an unchecked one going stale).
pub(crate) const AGE_CHECK_INTERVAL: Duration = Duration::from_secs(1);

/// What a stat of the store file found: enough to tell that it was
/// rewritten. Writers rename a new file into place, so a rewrite changes
/// the inode even when the time and size do not.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileStamp {
    Missing,
    Present {
        modified: Option<SystemTime>,
        len: u64,
        inode: u64,
    },
}

/// The file system the overlay reads the store through.
pub(crate) trait StoreFiles {
    /// The stat of the file at `path`; [`FileStamp::Missing`] when there
    /// is none or it cannot be stat'ed.
    fn stamp(&self, path: &Path) -> FileStamp;

    /// The text of the file at `path`, or `None` when it cannot be read.
    fn read(&self, path: &Path) -> Option<String>;
}

/// This machine's file system.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SystemFiles;

impl StoreFiles for SystemFiles {
    fn stamp(&self, path: &Path) -> FileStamp {
        let Ok(metadata) = std::fs::metadata(path) else {
            return FileStamp::Missing;
        };
        #[cfg(unix)]
        let inode = std::os::unix::fs::MetadataExt::ino(&metadata);
        #[cfg(not(unix))]
        let inode = 0;
        FileStamp::Present {
            modified: metadata.modified().ok(),
            len: metadata.len(),
            inode,
        }
    }

    fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
}

/// The live table the TUI's rows read, kept current from the store
/// between snapshots. See the module documentation.
pub(crate) struct LiveOverlay {
    /// Where the store is read from; `None` while the overlay is off.
    source: Option<Source>,
    /// The store bound to the last snapshot it was bound to.
    table: BTreeMap<NodeId, LiveBinding>,
    /// The cells of the active table at the last age check.
    checked: BTreeMap<NodeId, LiveCell>,
    checked_at: Option<Instant>,
}

/// The store file and what was last read from it.
struct Source {
    path: PathBuf,
    files: Box<dyn StoreFiles>,
    probe: Box<dyn PidProbe>,
    /// The stamp of the file [`Self::file`] was read from; `None` before
    /// the first read.
    read_stamp: Option<FileStamp>,
    read_at: Option<Instant>,
    file: LiveFile,
}

impl Source {
    /// Read the file whose stat was `stamp`. A missing file is not read.
    fn read(&mut self, stamp: FileStamp, now: Instant) {
        self.file = match stamp {
            FileStamp::Missing => LiveFile::default(),
            FileStamp::Present { .. } => self
                .files
                .read(&self.path)
                .map_or_else(LiveFile::default, |text| LiveFile::read_text(&text)),
        };
        self.read_stamp = Some(stamp);
        self.read_at = Some(now);
    }
}

impl LiveOverlay {
    /// An overlay that is off: rows read each snapshot's own table.
    pub(crate) fn off() -> Self {
        Self {
            source: None,
            table: BTreeMap::new(),
            checked: BTreeMap::new(),
            checked_at: None,
        }
    }

    /// The overlay over this user's store (`$XDG_STATE_HOME`), checked
    /// against this machine's process table; off when there is no state
    /// home.
    pub(crate) fn from_env() -> Self {
        LiveStore::from_env().map_or_else(Self::off, |store| {
            Self::over(
                store.path(),
                Box::new(SystemFiles),
                Box::new(ProcProbe::system()),
            )
        })
    }

    /// The overlay over the store file at `path`, read through `files`,
    /// with each bound entry's process checked through `probe`.
    pub(crate) fn over(
        path: impl Into<PathBuf>,
        files: Box<dyn StoreFiles>,
        probe: Box<dyn PidProbe>,
    ) -> Self {
        Self {
            source: Some(Source {
                path: path.into(),
                files,
                probe,
                read_stamp: None,
                read_at: None,
                file: LiveFile::default(),
            }),
            ..Self::off()
        }
    }

    /// Whether the overlay is on.
    pub(crate) fn is_on(&self) -> bool {
        self.source.is_some()
    }

    /// The live table the rows of `snapshot` read: the overlay's own when
    /// it is on, else the snapshot's.
    pub(crate) fn table<'a>(
        &'a self,
        snapshot: &'a GraphSnapshot,
    ) -> &'a BTreeMap<NodeId, LiveBinding> {
        if self.is_on() {
            &self.table
        } else {
            &snapshot.node_live
        }
    }

    /// Bind the store to `snapshot`, which is about to replace the one
    /// the TUI holds, from what was last read of it (the store is read
    /// first when it never was). The next [`Self::poll`] picks up any
    /// rewrite since.
    pub(crate) fn rebind(&mut self, snapshot: &GraphSnapshot, now: Instant) {
        let Some(source) = &mut self.source else {
            return;
        };
        if source.read_stamp.is_none() {
            let stamp = source.files.stamp(&source.path);
            source.read(stamp, now);
        }
        self.bind(snapshot);
    }

    /// One tick: stat the store, and when it was rewritten since it was
    /// last read and [`REREAD_INTERVAL`] has passed, read it and bind it
    /// to `snapshot` (the one the TUI holds, if any). Returns whether the
    /// table changed.
    pub(crate) fn poll(&mut self, now: Instant, snapshot: Option<&GraphSnapshot>) -> bool {
        let Some(source) = &mut self.source else {
            return false;
        };
        let stamp = source.files.stamp(&source.path);
        if source.read_stamp == Some(stamp) {
            return false;
        }
        if source
            .read_at
            .is_some_and(|at| now.saturating_duration_since(at) < REREAD_INTERVAL)
        {
            return false;
        }
        source.read(stamp, now);
        snapshot.is_some_and(|snapshot| self.bind(snapshot))
    }

    /// Whether time alone changed what a row of `snapshot` would show
    /// since the last check, checked at most once per
    /// [`AGE_CHECK_INTERVAL`]: the cells of the active table at `now_ms`
    /// against those of the last check. Reads no file and no process.
    pub(crate) fn aged(&mut self, snapshot: &GraphSnapshot, now: Instant, now_ms: i64) -> bool {
        if self
            .checked_at
            .is_some_and(|at| now.saturating_duration_since(at) < AGE_CHECK_INTERVAL)
        {
            return false;
        }
        self.checked_at = Some(now);
        let cells = cells_at(self.table(snapshot), now_ms);
        let moved = cells != self.checked;
        self.checked = cells;
        moved
    }

    /// Bind what was last read to `snapshot`. Returns whether the table
    /// changed.
    fn bind(&mut self, snapshot: &GraphSnapshot) -> bool {
        let Some(source) = &self.source else {
            return false;
        };
        let table = bind_live(snapshot, &source.file, source.probe.as_ref());
        let changed = table != self.table;
        self.table = table;
        changed
    }
}

impl Default for LiveOverlay {
    fn default() -> Self {
        Self::off()
    }
}

impl fmt::Debug for LiveOverlay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveOverlay")
            .field("on", &self.is_on())
            .field("path", &self.source.as_ref().map(|source| &source.path))
            .field("entries", &self.table.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "live_overlay_tests.rs"]
mod tests;
