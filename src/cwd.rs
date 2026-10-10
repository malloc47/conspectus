//! The process working directory as an optional input (ADR 0111).
//!
//! The launch directory is a convenient default (an extra scan root,
//! the project-config anchor, the target of authoring commands), but
//! it can disappear while Conspectus runs, and `current_dir()` then
//! fails with `ENOENT`. Everything reads it through this module so a
//! missing directory degrades instead of failing the command.
//! `tests/cwd_hygiene.rs` keeps other code from calling
//! `std::env::current_dir()` directly.

use std::path::{Path, PathBuf};

/// The working directory, when it still exists as a directory.
pub fn current() -> Option<PathBuf> {
    std::env::current_dir().ok().filter(|path| path.is_dir())
}

/// The working directory as the default target of an authoring
/// command. When it is gone, the error names `flag`, the option that
/// supplies the target explicitly.
pub fn for_default_target(flag: &str) -> anyhow::Result<PathBuf> {
    current().ok_or_else(|| {
        anyhow::anyhow!(
            "the current directory is no longer available (was it deleted?); \
             pass {flag} to choose a target explicitly"
        )
    })
}

/// Scan roots for a long-running process. Explicit roots (flags or
/// config) are used as given; the launch directory, captured at
/// startup, is used only while it still exists, so deleting it after
/// launch drops it from the next discovery run instead of failing it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanRoots {
    explicit: Vec<PathBuf>,
    launch_dir: Option<PathBuf>,
}

impl ScanRoots {
    /// Explicit roots, used as given.
    pub fn explicit(roots: Vec<PathBuf>) -> Self {
        Self {
            explicit: roots,
            launch_dir: None,
        }
    }

    /// The launch directory as an optional root. `None` (the directory
    /// was already gone at startup) yields no roots.
    pub fn launch_dir(dir: Option<PathBuf>) -> Self {
        Self {
            explicit: Vec::new(),
            launch_dir: dir,
        }
    }

    /// `flags` when given, else `configured`, else `launch_dir`.
    pub fn resolve(
        flags: Vec<PathBuf>,
        configured: &[PathBuf],
        launch_dir: Option<PathBuf>,
    ) -> Self {
        if !flags.is_empty() {
            Self::explicit(flags)
        } else if !configured.is_empty() {
            Self::explicit(configured.to_vec())
        } else {
            Self::launch_dir(launch_dir)
        }
    }

    /// The roots for one discovery run.
    pub fn effective(&self) -> Vec<PathBuf> {
        if !self.explicit.is_empty() {
            return self.explicit.clone();
        }
        self.launch_dir
            .iter()
            .filter(|dir| dir.is_dir())
            .cloned()
            .collect()
    }

    /// The captured launch directory, if roots fell back to it.
    pub fn launch_dir_root(&self) -> Option<&Path> {
        self.launch_dir.as_deref()
    }
}

#[cfg(test)]
#[path = "cwd_tests.rs"]
mod tests;
