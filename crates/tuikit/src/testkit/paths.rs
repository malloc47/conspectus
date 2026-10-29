//! An in-memory file system for testing path fields: a tree of
//! directories and files, a home directory, and a count of every question
//! a field asks it.
//!
//! A [`PathField`](crate::path_field::PathField) reads paths only through
//! a [`PathSource`]; [`MemoryPaths`] is one a test builds in a line, so it
//! needs no real directory and no home of its own, and can check that a
//! field asks nothing while it is drawn ([`MemoryPaths::calls`]).
//!
//! ```
//! use std::sync::Arc;
//! use tuikit::path_field::{PathField, PathKind};
//! use tuikit::testkit::paths::MemoryPaths;
//! use tuikit::theme::Theme;
//!
//! let paths = Arc::new(MemoryPaths::new().with_home("/home/me").dir("/home/me/src"));
//! let field = PathField::new("~/s", PathKind::Dir, paths.clone());
//! assert_eq!(field.completion().map(|c| c.value()), Some("~/src"));
//!
//! let asked = paths.calls();
//! let _ = field.view().line(40, &Theme::default());
//! assert_eq!(paths.calls(), asked, "drawing asks the source nothing");
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::path_field::{Entry, PathKind, PathSource};

/// How many times each of a [`PathSource`]'s questions was asked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Calls {
    /// Directory listings ([`PathSource::entries`]).
    pub entries: usize,
    /// Kind lookups ([`PathSource::kind`]).
    pub kind: usize,
    /// Home directory lookups ([`PathSource::home`]).
    pub home: usize,
}

impl Calls {
    /// Every question asked, of any kind.
    pub fn total(self) -> usize {
        self.entries + self.kind + self.home
    }
}

/// A file system in memory: absolute paths, each a directory or a file,
/// with every ancestor of a path a directory, and an optional home.
///
/// Paths are compared by their components, so `/a/b/` and `/a//b` are
/// `/a/b`. A relative path names nothing.
#[derive(Debug, Default)]
pub struct MemoryPaths {
    tree: BTreeMap<PathBuf, PathKind>,
    home: Option<PathBuf>,
    entries: AtomicUsize,
    kind: AtomicUsize,
    home_asked: AtomicUsize,
}

impl MemoryPaths {
    /// An empty file system: only the root directory, and no home.
    pub fn new() -> Self {
        let mut paths = Self::default();
        paths.tree.insert(PathBuf::from("/"), PathKind::Dir);
        paths
    }

    /// The same file system with the directory `path`, and its ancestors.
    #[must_use]
    pub fn dir(mut self, path: impl AsRef<Path>) -> Self {
        self.add(path.as_ref(), PathKind::Dir);
        self
    }

    /// The same file system with the file `path`, and its ancestors as
    /// directories.
    #[must_use]
    pub fn file(mut self, path: impl AsRef<Path>) -> Self {
        self.add(path.as_ref(), PathKind::File);
        self
    }

    /// The same file system with `path` as the home directory (and a
    /// directory).
    #[must_use]
    pub fn with_home(mut self, path: impl AsRef<Path>) -> Self {
        self.add(path.as_ref(), PathKind::Dir);
        self.home = Some(normal(path.as_ref()));
        self
    }

    /// How many times each question has been asked so far.
    pub fn calls(&self) -> Calls {
        Calls {
            entries: self.entries.load(Ordering::Relaxed),
            kind: self.kind.load(Ordering::Relaxed),
            home: self.home_asked.load(Ordering::Relaxed),
        }
    }

    fn add(&mut self, path: &Path, kind: PathKind) {
        let path = normal(path);
        for ancestor in path.ancestors().skip(1) {
            if !ancestor.as_os_str().is_empty() {
                self.tree.insert(ancestor.to_path_buf(), PathKind::Dir);
            }
        }
        self.tree.insert(path, kind);
    }
}

impl PathSource for MemoryPaths {
    fn entries(&self, dir: &Path) -> Vec<Entry> {
        self.entries.fetch_add(1, Ordering::Relaxed);
        let dir = normal(dir);
        if self.tree.get(&dir) != Some(&PathKind::Dir) {
            return Vec::new();
        }
        self.tree
            .iter()
            .filter(|(path, _)| path.parent() == Some(dir.as_path()))
            .filter_map(|(path, kind)| {
                let name = path.file_name()?.to_str()?;
                Some(Entry::new(name, *kind))
            })
            .collect()
    }

    fn kind(&self, path: &Path) -> Option<PathKind> {
        self.kind.fetch_add(1, Ordering::Relaxed);
        self.tree.get(&normal(path)).copied()
    }

    fn home(&self) -> Option<PathBuf> {
        self.home_asked.fetch_add(1, Ordering::Relaxed);
        self.home.clone()
    }
}

/// `path` by its components: no trailing or doubled separator.
fn normal(path: &Path) -> PathBuf {
    path.components().collect()
}
