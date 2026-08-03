//! Worktree backend seam (H-WT-002, ADR 0092).
//!
//! Mirrors the mux-backend / forge-adapter registries: a
//! [`WorktreeBackend`] trait with a required read-only [`list`] plus a
//! [`capabilities`] report, and swappable implementations. This module
//! ships the always-available, ADR-0087-clean **git** backend, which
//! enumerates a repo's worktrees via `git worktree list --porcelain`
//! and never mutates git state. The rich mutation backend
//! (`worktrunk`, create/remove) lands in H-WT-003 as a second
//! implementation of the same trait.
//!
//! [`list`]: WorktreeBackend::list
//! [`capabilities`]: WorktreeBackend::capabilities

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

use crate::model::WorktreeKind;

/// Backend identifier for the built-in thin git backend. Distinct from
/// the `git` *provider* key (`providers::GIT`): this names which
/// worktree backend produced a record, the way `tmux` / `zellij` name
/// mux backends. `worktrunk` joins as a second key in H-WT-003.
pub const GIT_BACKEND: &str = "git";

/// What a worktree backend can do beyond read-only `list`. The built-in
/// git backend reports both `false` (ADR 0092: it never mutates git
/// state); only an external dedicated tool like `worktrunk` reports
/// `can_create` / `can_remove`.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct WorktreeCaps {
    pub can_create: bool,
    pub can_remove: bool,
}

/// One worktree as reported by a backend's `list`. Maps onto a
/// [`crate::model::CheckoutNode`] during discovery (H-WT-002 step 3).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeRecord {
    /// Absolute path to the worktree's working directory.
    pub path: String,
    /// Full branch ref (`refs/heads/...`) this worktree checks out, or
    /// `None` for a detached HEAD or a bare entry.
    pub branch: Option<String>,
    /// Checked-out commit oid, when reported.
    pub head: Option<String>,
    /// Primary (the repo's main working tree) vs linked worktree.
    pub kind: WorktreeKind,
    /// `Some(reason)` when git reports the worktree locked; the reason
    /// may be empty when git gives none.
    pub locked: Option<String>,
    /// `Some(reason)` when git reports the worktree prunable.
    pub prunable: Option<String>,
    /// True for a bare main worktree (no working tree / HEAD).
    pub bare: bool,
    /// True when HEAD is detached (no branch).
    pub detached: bool,
}

/// A pluggable worktree backend (ADR 0092). `list` is required and
/// read-only; mutation (`create` / `remove`) is added by the external
/// `worktrunk` backend in H-WT-003 and gated behind
/// [`WorktreeCaps`].
pub trait WorktreeBackend: Send + Sync {
    /// Stable key identifying this backend (`git`, `worktrunk`).
    fn backend_key(&self) -> &'static str;

    /// What this backend can do beyond read-only listing.
    fn capabilities(&self) -> WorktreeCaps;

    /// Enumerate every worktree of the repo containing `repo_root`.
    /// Read-only. Returns an empty vec (not an error) when the path is
    /// not a git repo or the backend binary is absent, so discovery
    /// degrades cleanly on sparse hosts.
    fn list(&self, repo_root: &Path) -> Result<Vec<WorktreeRecord>>;
}

/// The built-in thin git backend: `git worktree list --porcelain`.
/// Read/list only — `capabilities` reports no mutation, and there is
/// no `create` / `remove` path here (ADR 0087 prohibition 6 stays
/// intact for the built-in backend).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemGitWorktree {
    binary: PathBuf,
}

impl Default for SystemGitWorktree {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("git"),
        }
    }
}

impl SystemGitWorktree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }
}

impl WorktreeBackend for SystemGitWorktree {
    fn backend_key(&self) -> &'static str {
        GIT_BACKEND
    }

    fn capabilities(&self) -> WorktreeCaps {
        WorktreeCaps::default()
    }

    fn list(&self, repo_root: &Path) -> Result<Vec<WorktreeRecord>> {
        let output = Command::new(&self.binary)
            .arg("-C")
            .arg(repo_root)
            .args(["worktree", "list", "--porcelain"])
            .output();

        let output = match output {
            Ok(output) => output,
            // No git binary → nothing to enumerate; degrade to empty
            // rather than failing the whole discovery pass.
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to spawn git binary at {}", self.binary.display())
                });
            }
        };

        // A non-zero exit means "not a git repo" (or a transient git
        // error). Either way there are no worktrees to surface; treat
        // it as an empty list, consistent with how the single-checkout
        // git probe returns `None` for non-repos.
        if !output.status.success() {
            return Ok(Vec::new());
        }

        Ok(parse_worktree_porcelain(&String::from_utf8_lossy(
            &output.stdout,
        )))
    }
}

/// Parse `git worktree list --porcelain` stdout into
/// [`WorktreeRecord`]s.
///
/// Records are newline-separated blocks terminated by a blank line.
/// Each block starts with `worktree <path>` and carries attribute
/// lines: `HEAD <oid>`, `branch <ref>`, `detached`, `bare`, `locked
/// [<reason>]`, `prunable [<reason>]`. Git lists the **main** worktree
/// first, so the first block is [`WorktreeKind::Primary`] and the rest
/// are [`WorktreeKind::Linked`].
pub fn parse_worktree_porcelain(stdout: &str) -> Vec<WorktreeRecord> {
    let mut records = Vec::new();
    let mut current: Option<WorktreeRecord> = None;

    let flush = |current: &mut Option<WorktreeRecord>, records: &mut Vec<WorktreeRecord>| {
        if let Some(mut record) = current.take() {
            // Kind is positional: the first worktree git lists is the
            // repo's main working tree.
            record.kind = if records.is_empty() {
                WorktreeKind::Primary
            } else {
                WorktreeKind::Linked
            };
            records.push(record);
        }
    };

    for line in stdout.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            flush(&mut current, &mut records);
            continue;
        }
        let (key, rest) = match line.split_once(' ') {
            Some((key, rest)) => (key, rest.trim()),
            None => (line, ""),
        };
        match key {
            "worktree" => {
                // A new block starts; flush any in-progress record
                // first (defensive — blocks are normally blank-line
                // separated).
                flush(&mut current, &mut records);
                current = Some(WorktreeRecord {
                    path: rest.to_string(),
                    branch: None,
                    head: None,
                    kind: WorktreeKind::Linked,
                    locked: None,
                    prunable: None,
                    bare: false,
                    detached: false,
                });
            }
            "HEAD" => {
                if let Some(record) = current.as_mut() {
                    record.head = Some(rest.to_string());
                }
            }
            "branch" => {
                if let Some(record) = current.as_mut() {
                    record.branch = Some(rest.to_string());
                }
            }
            "detached" => {
                if let Some(record) = current.as_mut() {
                    record.detached = true;
                }
            }
            "bare" => {
                if let Some(record) = current.as_mut() {
                    record.bare = true;
                }
            }
            "locked" => {
                if let Some(record) = current.as_mut() {
                    record.locked = Some(unquote_reason(rest));
                }
            }
            "prunable" => {
                if let Some(record) = current.as_mut() {
                    record.prunable = Some(unquote_reason(rest));
                }
            }
            _ => {}
        }
    }
    // Trailing record with no terminating blank line.
    flush(&mut current, &mut records);
    records
}

/// Best-effort strip of git's C-quoting around a lock/prune reason.
/// Git emits the reason bare or double-quoted; we drop a surrounding
/// pair of quotes and leave the rest as-is (reasons are display-only).
fn unquote_reason(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
#[path = "worktree_tests.rs"]
mod tests;
