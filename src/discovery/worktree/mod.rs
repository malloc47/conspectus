//! Worktree backend seam (H-WT-002, ADR 0092).
//!
//! Mirrors the mux-backend / forge-adapter registries: a
//! [`WorktreeBackend`] trait with a required read-only [`list`] plus a
//! [`capabilities`] report, and swappable implementations. This module
//! ships the always-available, ADR-0087-clean **git** backend, which
//! enumerates a repo's worktrees via `git worktree list --porcelain`
//! and never mutates git state, plus the **worktrunk** mutation
//! backend (H-WT-003), which shells out to the `wt` CLI for
//! `create` / `remove` — a category-4 subprocess launch (ADR 0087),
//! so the git mutation happens inside worktrunk, not Conspectus.
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

/// Backend identifier for the worktrunk mutation backend (H-WT-003).
pub const WORKTRUNK_BACKEND: &str = "worktrunk";

/// Which backend performs worktree *mutation* for the CLI / TUI
/// (H-WT-003). Discovery always lists via the git backend; this only
/// governs create / remove.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub enum WorktreeBackendSelection {
    /// Use worktrunk when `wt` is on PATH, otherwise stay read-only.
    #[default]
    Auto,
    /// Never mutate — read-only even when `wt` is present.
    Git,
    /// Require worktrunk; error when `wt` is absent.
    Worktrunk,
}

impl WorktreeBackendSelection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Git => "git",
            Self::Worktrunk => "worktrunk",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "auto" => Some(Self::Auto),
            "git" => Some(Self::Git),
            "worktrunk" => Some(Self::Worktrunk),
            _ => None,
        }
    }
}

/// Whether the `wt` (worktrunk) binary is resolvable on `$PATH`.
/// Scans `PATH` for an executable `wt` rather than spawning it, so the
/// check is cheap and side-effect-free.
pub fn worktrunk_available() -> bool {
    binary_on_path("wt")
}

fn binary_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

/// Resolve the mutation backend for CLI / TUI use from the config
/// selection and worktrunk availability (H-WT-003):
///
/// - [`WorktreeBackendSelection::Git`] → `Ok(None)` (read-only).
/// - [`WorktreeBackendSelection::Auto`] → the worktrunk backend when
///   available, else `Ok(None)`.
/// - [`WorktreeBackendSelection::Worktrunk`] → the worktrunk backend,
///   or `Err` when `wt` is not on PATH.
///
/// `available` is passed in (not probed here) so the decision is
/// unit-testable; production callers pass [`worktrunk_available`].
pub fn resolve_mutation_backend(
    selection: WorktreeBackendSelection,
    available: bool,
) -> Result<Option<Box<dyn WorktreeBackend>>> {
    match selection {
        WorktreeBackendSelection::Git => Ok(None),
        WorktreeBackendSelection::Auto => {
            Ok(available.then(|| Box::new(WorktrunkBackend::new()) as Box<dyn WorktreeBackend>))
        }
        WorktreeBackendSelection::Worktrunk => {
            if available {
                Ok(Some(Box::new(WorktrunkBackend::new())))
            } else {
                anyhow::bail!(
                    "`[worktree] backend = \"worktrunk\"` but the `wt` binary is not on PATH; \
                     install worktrunk (https://github.com/max-sixty/worktrunk) or set \
                     `backend = \"auto\"`"
                )
            }
        }
    }
}

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

/// Operator request to create a worktree (H-WT-003).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeCreateRequest {
    /// A path inside the repo the worktree belongs to.
    pub repo_root: PathBuf,
    /// Branch to create the worktree for (created fresh).
    pub branch: String,
    /// Base ref to branch from. `None` uses the backend default (the
    /// repo's default branch).
    pub base: Option<String>,
}

/// Operator request to remove a worktree (H-WT-003).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorktreeRemoveRequest {
    /// A path inside the repo the worktree belongs to.
    pub repo_root: PathBuf,
    /// Branch whose worktree should be removed.
    pub branch: String,
    /// Force removal even when the worktree holds untracked files
    /// (maps to worktrunk `-f`). Distinct from Conspectus's
    /// live-session guard, which is a CLI/TUI concern.
    pub force: bool,
}

/// Outcome of a worktree mutation (H-WT-003).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorktreeMutationOutcome {
    /// The mutation succeeded. `path` carries the affected worktree
    /// path when the backend reports it, else `None` (the caller can
    /// re-discover to find a freshly created worktree).
    Succeeded { path: Option<String> },
    /// This backend is read-only and does not perform the operation.
    Unsupported,
    /// The backend ran but the operation failed.
    Failed { code: Option<i32>, message: String },
}

/// A pluggable worktree backend (ADR 0092). `list` is required and
/// read-only; `create` / `remove` are mutation, default to
/// `Unsupported` (the read-only git backend), and are implemented by
/// the external `worktrunk` backend (H-WT-003) as a category-4
/// subprocess launch (ADR 0087). [`WorktreeCaps`] advertises which a
/// given backend supports.
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

    /// Create a worktree. Defaults to [`WorktreeMutationOutcome::Unsupported`]
    /// for read-only backends. `Err` is reserved for a failure to
    /// *launch* the backend; an operation that ran and failed is a
    /// [`WorktreeMutationOutcome::Failed`].
    fn create(&self, _req: &WorktreeCreateRequest) -> Result<WorktreeMutationOutcome> {
        Ok(WorktreeMutationOutcome::Unsupported)
    }

    /// Remove a worktree. Defaults to
    /// [`WorktreeMutationOutcome::Unsupported`].
    fn remove(&self, _req: &WorktreeRemoveRequest) -> Result<WorktreeMutationOutcome> {
        Ok(WorktreeMutationOutcome::Unsupported)
    }
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

/// Subprocess seam for the `wt` binary so [`WorktrunkBackend`] argv can
/// be unit-tested without spawning a real process.
pub trait WtRunner: Send + Sync {
    /// Run `wt <args>` and return its captured output.
    fn run(&self, args: &[&str]) -> io::Result<std::process::Output>;
}

/// Production [`WtRunner`] spawning the `wt` binary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemWt {
    binary: PathBuf,
}

impl Default for SystemWt {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("wt"),
        }
    }
}

impl SystemWt {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_binary(binary: impl Into<PathBuf>) -> Self {
        Self {
            binary: binary.into(),
        }
    }
}

impl WtRunner for SystemWt {
    fn run(&self, args: &[&str]) -> io::Result<std::process::Output> {
        Command::new(&self.binary).args(args).output()
    }
}

/// The worktrunk mutation backend (H-WT-003, ADR 0092). Shells out to
/// the `wt` CLI for `create` / `remove` — a category-4
/// Conspectus-constructed subprocess launch (ADR 0087); the git
/// mutation happens inside worktrunk, the dedicated tool, so
/// prohibition 6 stays intact. `list` delegates to the git porcelain
/// path (worktrunk worktrees are ordinary git worktrees).
pub struct WorktrunkBackend {
    runner: Box<dyn WtRunner>,
    git: SystemGitWorktree,
}

impl Default for WorktrunkBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WorktrunkBackend {
    pub fn new() -> Self {
        Self {
            runner: Box::new(SystemWt::new()),
            git: SystemGitWorktree::new(),
        }
    }

    /// Inject a fake `wt` runner for tests.
    pub fn with_runner(runner: Box<dyn WtRunner>) -> Self {
        Self {
            runner,
            git: SystemGitWorktree::new(),
        }
    }

    fn interpret(output: io::Result<std::process::Output>) -> Result<WorktreeMutationOutcome> {
        let output =
            output.map_err(|err| anyhow::anyhow!("failed to spawn worktrunk `wt`: {err}"))?;
        if output.status.success() {
            Ok(WorktreeMutationOutcome::Succeeded { path: None })
        } else {
            let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Ok(WorktreeMutationOutcome::Failed {
                code: output.status.code(),
                message,
            })
        }
    }
}

impl WorktreeBackend for WorktrunkBackend {
    fn backend_key(&self) -> &'static str {
        WORKTRUNK_BACKEND
    }

    fn capabilities(&self) -> WorktreeCaps {
        WorktreeCaps {
            can_create: true,
            can_remove: true,
        }
    }

    fn list(&self, repo_root: &Path) -> Result<Vec<WorktreeRecord>> {
        // Worktrunk worktrees are git worktrees; reuse the porcelain
        // enumeration rather than parsing `wt list` output.
        self.git.list(repo_root)
    }

    fn create(&self, req: &WorktreeCreateRequest) -> Result<WorktreeMutationOutcome> {
        // wt -C <repo> switch --create --no-cd [--base <ref>] <branch>
        let repo = req.repo_root.to_string_lossy();
        let mut args: Vec<&str> = vec!["-C", &repo, "switch", "--create", "--no-cd"];
        if let Some(base) = &req.base {
            args.push("--base");
            args.push(base);
        }
        args.push(&req.branch);
        Self::interpret(self.runner.run(&args))
    }

    fn remove(&self, req: &WorktreeRemoveRequest) -> Result<WorktreeMutationOutcome> {
        // wt -C <repo> remove --yes --foreground [--force] <branch>
        let repo = req.repo_root.to_string_lossy();
        let mut args: Vec<&str> = vec!["-C", &repo, "remove", "--yes", "--foreground"];
        if req.force {
            args.push("--force");
        }
        args.push(&req.branch);
        Self::interpret(self.runner.run(&args))
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
