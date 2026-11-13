//! The file system the TUI's path fields see: the framework's
//! `PathSource` over `std::fs` and `$HOME`.
//!
//! The framework's `PathField` reads no file system of its own, so every
//! path field here (the new mux and mux launch cwd, the launch form's
//! worktree repo, and the pin edit form's path rows) is built over this
//! source. It only reads: a directory's entries, what a path is, and
//! `$HOME`. A relative path is looked up as given, so it resolves against
//! the process's directory the way the commands the forms run will.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use tuikit::path_field::{Candidate, Entry, PathField, PathKind, PathSource};

/// The real file system, read only.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPaths;

impl PathSource for SystemPaths {
    fn entries(&self, dir: &Path) -> Vec<Entry> {
        let Ok(read) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        read.flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                // The entry's own type comes with the listing; only a link
                // needs a look at what it leads to, and a broken one is
                // nothing a field could complete to.
                let file_type = entry.file_type().ok()?;
                let is_dir = if file_type.is_symlink() {
                    std::fs::metadata(entry.path()).ok()?.is_dir()
                } else {
                    file_type.is_dir()
                };
                Some(Entry::new(name, kind_of(is_dir)))
            })
            .collect()
    }

    fn kind(&self, path: &Path) -> Option<PathKind> {
        std::fs::metadata(path)
            .ok()
            .map(|meta| kind_of(meta.is_dir()))
    }

    fn home(&self) -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn kind_of(is_dir: bool) -> PathKind {
    if is_dir {
        PathKind::Dir
    } else {
        PathKind::File
    }
}

/// The one source every path field shares.
pub fn system() -> Arc<dyn PathSource + Send + Sync> {
    static SYSTEM: LazyLock<Arc<SystemPaths>> = LazyLock::new(|| Arc::new(SystemPaths));
    SYSTEM.clone()
}

/// A field for a directory on the real file system, holding `value` and
/// offering `candidates` first.
pub fn directory_field(value: impl Into<String>, candidates: Vec<Candidate>) -> PathField {
    PathField::new(value, PathKind::Dir, system()).with_candidates(candidates)
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
