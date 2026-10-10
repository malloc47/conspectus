//! `conspectus worktree` subcommand (ADR 0092).
//!
//! `worktree list` renders the git worktrees discovery found
//! (read-only). `worktree new` / `rm` delegate to the configured
//! mutation backend (worktrunk) — a category-4 subprocess launch
//! (ADR 0087); `rm` refuses to remove a worktree hosting a live
//! agent/mux session unless `--force`.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use clap::{Args, Subcommand};

use crate::config::{ConfigLoader, parse_duration_short};
use crate::discovery::tmux::SystemTmux;
use crate::discovery::tmux::teardown::SystemSignaller;
use crate::discovery::worktree::close_down::{
    CloseDownPlan, CloseDownReport, execute_close_down, plan_close_down,
};
use crate::discovery::worktree::{
    WorktreeCreateRequest, WorktreeMergeRequest, WorktreeMutationOutcome, WorktreePruneRequest,
    WorktreeRemoveRequest, resolve_mutation_backend, worktrunk_available,
};
use crate::model::{GraphNode, GraphSnapshot, RepoId, WorktreeKind, path_is_ancestor_of};

use super::discover_for_store_selection;

#[derive(Debug, Args)]
pub struct WorktreeArgs {
    #[command(subcommand)]
    command: WorktreeCommand,
}

#[derive(Debug, Subcommand)]
enum WorktreeCommand {
    /// List the git worktrees discovered across the scanned repos.
    List(WorktreeListArgs),
    /// Create a worktree + branch (delegates to the mutation backend).
    /// Does not launch anything — pin/attach separately.
    New(WorktreeNewArgs),
    /// Remove a worktree. Refuses when it hosts a live session unless
    /// `--force`.
    Rm(WorktreeRmArgs),
    /// Merge a worktree's branch back (squash + rebase + fast-forward)
    /// and remove the worktree. Guarded like `rm`.
    Merge(WorktreeMergeArgs),
    /// Close down a stream of work: optionally merge or
    /// discard the branch, terminate the mux/agent sessions rooted in
    /// the worktree, remove the worktree, and drop pins rooted there.
    Close(WorktreeCloseArgs),
    /// Prune worktrees already merged into the repo's default branch
    /// (delegates to `wt step prune`). Distinct from git's
    /// stale-admin prune.
    Prune(WorktreePruneArgs),
}

impl WorktreeArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            WorktreeCommand::List(args) => args.run(),
            WorktreeCommand::New(args) => args.run(),
            WorktreeCommand::Rm(args) => args.run(),
            WorktreeCommand::Merge(args) => args.run(),
            WorktreeCommand::Close(args) => args.run(),
            WorktreeCommand::Prune(args) => args.run(),
        }
    }
}

/// Resolve the mutation backend from `[worktree] backend` + `wt`
/// availability, erroring clearly when none is available.
fn mutation_backend() -> Result<Box<dyn crate::discovery::worktree::WorktreeBackend>> {
    let outcome = ConfigLoader::from_env().load(crate::cwd::current().as_deref());
    match resolve_mutation_backend(outcome.config.worktree.backend, worktrunk_available())? {
        Some(backend) => Ok(backend),
        None => bail!(
            "no worktree mutation backend available (read-only). Install worktrunk \
             (https://github.com/max-sixty/worktrunk) or set `[worktree] backend`"
        ),
    }
}

#[derive(Debug, Args)]
struct WorktreeNewArgs {
    /// Branch to create the worktree for.
    branch: String,
    /// Base ref to branch from (defaults to the repo's default branch).
    #[arg(long)]
    base: Option<String>,
    /// A path inside the repo to operate on. Defaults to the current
    /// directory.
    #[arg(long = "repo", value_name = "PATH")]
    repo: Option<PathBuf>,
}

impl WorktreeNewArgs {
    fn run(self) -> Result<()> {
        let repo_root = match self.repo {
            Some(path) => path,
            None => crate::cwd::for_default_target("--repo <path>")?,
        };
        let backend = mutation_backend()?;
        let outcome = backend.create(&WorktreeCreateRequest {
            repo_root,
            branch: self.branch.clone(),
            base: self.base.clone(),
        })?;
        match outcome {
            WorktreeMutationOutcome::Succeeded { .. } => {
                println!("created worktree for branch `{}`", self.branch);
                Ok(())
            }
            WorktreeMutationOutcome::Unsupported => {
                bail!("the configured worktree backend cannot create worktrees")
            }
            WorktreeMutationOutcome::Failed { code, message } => {
                bail!("worktree create failed{}: {message}", code_suffix(code))
            }
        }
    }
}

#[derive(Debug, Args)]
struct WorktreeRmArgs {
    /// Branch whose worktree should be removed.
    branch: String,
    /// A path inside the repo to operate on. Defaults to the current
    /// directory.
    #[arg(long = "repo", value_name = "PATH")]
    repo: Option<PathBuf>,
    /// Remove even when the worktree hosts a live session and/or holds
    /// untracked files.
    #[arg(long, short)]
    force: bool,
}

impl WorktreeRmArgs {
    fn run(self) -> Result<()> {
        let repo_root = match &self.repo {
            Some(path) => path.clone(),
            None => crate::cwd::for_default_target("--repo <path>")?,
        };

        // Live-session guard: discover, find the worktree's path, and
        // refuse when a session is rooted in it (unless --force).
        let snapshot = discover_for_store_selection(std::slice::from_ref(&repo_root))?;
        if !self.force
            && let Some(path) = worktree_path_for_branch(&snapshot, &self.branch)
        {
            let sessions = live_sessions_in_worktree(&snapshot, &path);
            if !sessions.is_empty() {
                let mut msg = format!(
                    "worktree for `{}` hosts {} live session(s):",
                    self.branch,
                    sessions.len()
                );
                for s in &sessions {
                    msg.push_str(&format!("\n  - {s}"));
                }
                msg.push_str("\nre-run with --force to remove anyway");
                bail!(msg);
            }
        }

        let backend = mutation_backend()?;
        let outcome = backend.remove(&WorktreeRemoveRequest {
            repo_root,
            branch: self.branch.clone(),
            force: self.force,
        })?;
        match outcome {
            WorktreeMutationOutcome::Succeeded { .. } => {
                println!("removed worktree for branch `{}`", self.branch);
                Ok(())
            }
            WorktreeMutationOutcome::Unsupported => {
                bail!("the configured worktree backend cannot remove worktrees")
            }
            WorktreeMutationOutcome::Failed { code, message } => {
                bail!("worktree remove failed{}: {message}", code_suffix(code))
            }
        }
    }
}

#[derive(Debug, Args)]
struct WorktreeMergeArgs {
    /// Branch whose worktree should be merged back and removed.
    branch: String,
    /// Target branch to merge into (defaults to the repo's default).
    #[arg(long)]
    target: Option<String>,
    /// A path inside the repo to operate on. Defaults to the current
    /// directory.
    #[arg(long = "repo", value_name = "PATH")]
    repo: Option<PathBuf>,
    /// Merge even when the worktree hosts a live session.
    #[arg(long, short)]
    force: bool,
}

impl WorktreeMergeArgs {
    fn run(self) -> Result<()> {
        let repo_root = match &self.repo {
            Some(path) => path.clone(),
            None => crate::cwd::for_default_target("--repo <path>")?,
        };
        let snapshot = discover_for_store_selection(std::slice::from_ref(&repo_root))?;
        let Some(worktree_root) = worktree_path_for_branch(&snapshot, &self.branch) else {
            bail!(
                "no discovered worktree for branch `{}` under {}",
                self.branch,
                repo_root.display()
            );
        };
        // Merge removes the worktree, so guard live sessions like `rm`.
        if !self.force {
            let sessions = live_sessions_in_worktree(&snapshot, &worktree_root);
            if !sessions.is_empty() {
                let mut msg = format!(
                    "worktree for `{}` hosts {} live session(s):",
                    self.branch,
                    sessions.len()
                );
                for s in &sessions {
                    msg.push_str(&format!("\n  - {s}"));
                }
                msg.push_str("\nre-run with --force to merge & remove anyway");
                bail!(msg);
            }
        }

        let backend = mutation_backend()?;
        let outcome = backend.merge(&WorktreeMergeRequest {
            worktree_root: PathBuf::from(&worktree_root),
            target: self.target.clone(),
        })?;
        match outcome {
            WorktreeMutationOutcome::Succeeded { .. } => {
                println!(
                    "merged worktree for branch `{}` back and removed it",
                    self.branch
                );
                Ok(())
            }
            WorktreeMutationOutcome::Unsupported => {
                bail!("the configured worktree backend cannot merge worktrees")
            }
            WorktreeMutationOutcome::Failed { code, message } => {
                bail!("worktree merge failed{}: {message}", code_suffix(code))
            }
        }
    }
}

/// Interactive y/N prompt for close-down. Returns `true` only on an
/// explicit yes.
fn confirm_prompt(summary: &str) -> Result<bool> {
    print!("{summary}\nProceed? [y/N] ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[derive(Debug, Args)]
struct WorktreeCloseArgs {
    /// Branch whose stream of work should be closed down.
    branch: String,
    /// Merge the branch back before tearing down (mutually exclusive
    /// with `--discard`).
    #[arg(long, conflicts_with = "discard")]
    merge: bool,
    /// Discard the branch's work — remove the worktree without merging
    /// (mutually exclusive with `--merge`).
    #[arg(long)]
    discard: bool,
    /// Target branch for `--merge` (defaults to the repo's default).
    #[arg(long)]
    target: Option<String>,
    /// A path inside the repo to operate on. Defaults to the current
    /// directory.
    #[arg(long = "repo", value_name = "PATH")]
    repo: Option<PathBuf>,
    /// Skip the confirmation prompt regardless of `teardown_confirm`.
    #[arg(long, short = 'y')]
    yes: bool,
    /// Override `[worktree] teardown_grace` for this run (e.g. `5s`,
    /// `0s` to skip the graceful phase).
    #[arg(long)]
    grace: Option<String>,
}

impl WorktreeCloseArgs {
    fn run(self) -> Result<()> {
        if !self.merge && !self.discard {
            bail!("choose one of --merge (land the branch) or --discard (drop it)");
        }

        let repo_root = match &self.repo {
            Some(path) => path.clone(),
            None => crate::cwd::for_default_target("--repo <path>")?,
        };
        let config = ConfigLoader::from_env().load_from(&repo_root).config;

        let grace = match &self.grace {
            Some(raw) => parse_duration_short(raw)
                .map_err(|message| anyhow::anyhow!("invalid --grace `{raw}`: {message}"))?,
            None => config.worktree.teardown_grace,
        };

        let snapshot = discover_for_store_selection(std::slice::from_ref(&repo_root))?;
        let Some(plan) = plan_close_down(&snapshot, repo_root.clone(), &self.branch) else {
            bail!(
                "no discovered worktree for branch `{}` under {}",
                self.branch,
                repo_root.display()
            );
        };

        // Confirmation, per ADR 0093 policy (`--yes` overrides).
        if !self.yes
            && config
                .worktree
                .teardown_confirm
                .should_confirm(plan.has_live())
        {
            let mut summary = format!(
                "Close down `{}` ({}) at {}",
                self.branch,
                if self.merge { "merge" } else { "discard" },
                plan.worktree_root
            );
            if plan.has_live() {
                summary.push_str("\nEnds these live sessions (transcripts preserved):");
                for label in &plan.live_labels {
                    summary.push_str(&format!("\n  - {label}"));
                }
            }
            if !plan.pins.is_empty() {
                summary.push_str("\nDrops these pins:");
                for pin in &plan.pins {
                    summary.push_str(&format!("\n  - {}", pin.display_name));
                }
            }
            if !confirm_prompt(&summary)? {
                println!("aborted");
                return Ok(());
            }
        }

        let backend = mutation_backend()?;
        let report = execute_close_down(
            &plan,
            self.discard,
            self.target,
            backend.as_ref(),
            &SystemTmux::new(),
            &SystemSignaller,
            grace,
        )?;

        render_close_down_report(&plan, &report)
    }
}

#[derive(Debug, Args)]
struct WorktreePruneArgs {
    /// A path inside the repo to prune within. Defaults to the current
    /// directory.
    #[arg(long = "repo", value_name = "PATH")]
    repo: Option<PathBuf>,
    /// Show what would be removed without removing anything.
    #[arg(long)]
    dry_run: bool,
    /// Skip the confirmation prompt.
    #[arg(long, short = 'y')]
    yes: bool,
}

impl WorktreePruneArgs {
    fn run(self) -> Result<()> {
        let repo_root = match &self.repo {
            Some(path) => path.clone(),
            None => crate::cwd::for_default_target("--repo <path>")?,
        };

        // Removing merged worktrees is bulk + destructive, so confirm
        // unless it's a dry run or the operator passed --yes.
        if !self.dry_run && !self.yes {
            let summary = format!(
                "Prune worktrees merged into the default branch under {} \
                 (worktrunk skips worktrees younger than its --min-age)",
                repo_root.display()
            );
            if !confirm_prompt(&summary)? {
                println!("aborted");
                return Ok(());
            }
        }

        let backend = mutation_backend()?;
        let outcome = backend.prune(&WorktreePruneRequest {
            repo_root,
            dry_run: self.dry_run,
        })?;
        match outcome {
            WorktreeMutationOutcome::Succeeded { .. } => {
                if self.dry_run {
                    println!("dry run complete (nothing removed)");
                } else {
                    println!("pruned merged worktrees");
                }
                Ok(())
            }
            WorktreeMutationOutcome::Unsupported => {
                bail!("the configured worktree backend cannot prune worktrees")
            }
            WorktreeMutationOutcome::Failed { code, message } => {
                bail!("worktree prune failed{}: {message}", code_suffix(code))
            }
        }
    }
}

/// Emit the per-step CLI summary of a close-down, and surface a
/// merge/remove failure as an error exit.
fn render_close_down_report(plan: &CloseDownPlan, report: &CloseDownReport) -> Result<()> {
    use crate::discovery::tmux::TmuxKillOutcome;
    for (target, teardown) in &report.teardowns {
        let phase = if teardown.graceful_exited {
            "exited gracefully"
        } else if teardown.signalled {
            "terminated"
        } else {
            "killed"
        };
        match &teardown.kill {
            TmuxKillOutcome::Killed | TmuxKillOutcome::NoTarget => {
                println!("session `{target}` {phase}");
            }
            TmuxKillOutcome::Unsupported => {
                eprintln!(
                    "warning: backend cannot terminate `{target}`; close the session yourself"
                );
            }
            TmuxKillOutcome::Unavailable(reason) => {
                eprintln!(
                    "warning: could not terminate `{target}`: {}",
                    reason.as_str()
                );
            }
            TmuxKillOutcome::Failed { code, message } => {
                eprintln!(
                    "warning: could not terminate `{target}`{}: {message}",
                    code_suffix(*code)
                );
            }
        }
    }

    let verb = if report.landed { "merge" } else { "remove" };
    match &report.worktree {
        WorktreeMutationOutcome::Succeeded { .. } => {
            if report.landed {
                println!("merged `{}` back and removed its worktree", plan.branch);
            } else {
                println!("removed worktree for `{}`", plan.branch);
            }
        }
        WorktreeMutationOutcome::Unsupported => {
            bail!("the configured worktree backend cannot {verb} worktrees")
        }
        WorktreeMutationOutcome::Failed { code, message } => {
            bail!(
                "worktree {verb} for `{}` failed{}: {message}",
                plan.branch,
                code_suffix(*code)
            )
        }
    }

    for name in &report.dropped_pins {
        println!("dropped pin `{name}`");
    }
    for (name, err) in &report.pin_errors {
        eprintln!("warning: could not drop pin `{name}`: {err}");
    }
    Ok(())
}

fn code_suffix(code: Option<i32>) -> String {
    code.map(|c| format!(" (exit {c})")).unwrap_or_default()
}

/// Path of the worktree checking out `branch` (short name), from the
/// discovered graph. `None` when no enumerated worktree matches.
fn worktree_path_for_branch(snapshot: &GraphSnapshot, branch: &str) -> Option<String> {
    for node in &snapshot.nodes {
        let GraphNode::Checkout(checkout) = node else {
            continue;
        };
        if checkout.worktree.is_none() {
            continue;
        }
        if let Some(current) = &checkout.current_branch
            && short_branch(&current.refname) == branch
        {
            return Some(checkout.root.clone());
        }
    }
    None
}

/// Live agent / mux sessions rooted inside `worktree_path`. A session
/// counts as "in" the worktree when its cwd (or the mux's active pane
/// path) is at or under the worktree root.
fn live_sessions_in_worktree(snapshot: &GraphSnapshot, worktree_path: &str) -> Vec<String> {
    let root = Path::new(worktree_path);
    let mut sessions = Vec::new();
    for node in &snapshot.nodes {
        match node {
            GraphNode::AgentSession(session) => {
                if let Some(cwd) = &session.cwd
                    && path_is_ancestor_of(root, Path::new(cwd))
                {
                    sessions.push(format!("{} (agent)", session.harness_key));
                }
            }
            GraphNode::MuxSession(mux) => {
                let cwd = mux
                    .active_pane_current_path
                    .as_deref()
                    .or(mux.cwd.as_deref());
                if let Some(cwd) = cwd
                    && path_is_ancestor_of(root, Path::new(cwd))
                {
                    sessions.push(format!("{} (mux)", mux.native_id));
                }
            }
            _ => {}
        }
    }
    sessions
}

#[derive(Debug, Args)]
struct WorktreeListArgs {
    /// Root(s) used to discover repos and their worktrees. Defaults to
    /// the current directory.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl WorktreeListArgs {
    fn run(self) -> Result<()> {
        let snapshot = discover_for_store_selection(&self.scan_roots)?;
        print!("{}", render_worktrees(&collect_worktrees(&snapshot)));
        Ok(())
    }
}

/// One worktree row for display.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WorktreeRow {
    path: String,
    branch: Option<String>,
    kind: WorktreeKind,
    locked: bool,
    prunable: bool,
}

/// Group every worktree-bearing checkout by repo, primary first then
/// path-sorted. Only checkouts that carry worktree metadata (i.e. came
/// from git worktree enumeration) are included.
fn collect_worktrees(snapshot: &GraphSnapshot) -> BTreeMap<RepoId, Vec<WorktreeRow>> {
    let mut by_repo: BTreeMap<RepoId, Vec<WorktreeRow>> = BTreeMap::new();
    for node in &snapshot.nodes {
        let GraphNode::Checkout(checkout) = node else {
            continue;
        };
        let Some(meta) = &checkout.worktree else {
            continue;
        };
        by_repo
            .entry(checkout.id.repo.clone())
            .or_default()
            .push(WorktreeRow {
                path: checkout.root.clone(),
                branch: checkout
                    .current_branch
                    .as_ref()
                    .map(|b| short_branch(&b.refname)),
                kind: meta.kind,
                locked: meta.locked.is_some(),
                prunable: meta.prunable.is_some(),
            });
    }
    for rows in by_repo.values_mut() {
        rows.sort_by(|a, b| {
            primary_rank(a.kind)
                .cmp(&primary_rank(b.kind))
                .then_with(|| a.path.cmp(&b.path))
        });
    }
    by_repo
}

fn primary_rank(kind: WorktreeKind) -> u8 {
    match kind {
        WorktreeKind::Primary => 0,
        WorktreeKind::Linked => 1,
    }
}

/// Render the grouped worktrees as a per-repo block. Empty input
/// yields a single "no worktrees" line so the command is never silent.
fn render_worktrees(by_repo: &BTreeMap<RepoId, Vec<WorktreeRow>>) -> String {
    if by_repo.is_empty() {
        return "no worktrees found\n".to_string();
    }
    let mut out = String::new();
    for (repo, rows) in by_repo {
        out.push_str(&repo_display_name(repo));
        out.push('\n');
        let path_width = rows.iter().map(|r| r.path.len()).max().unwrap_or(0);
        let branch_width = rows
            .iter()
            .map(|r| r.branch.as_deref().unwrap_or("(detached)").len())
            .max()
            .unwrap_or(0);
        for row in rows {
            let branch = row.branch.as_deref().unwrap_or("(detached)");
            let kind = match row.kind {
                WorktreeKind::Primary => "primary",
                WorktreeKind::Linked => "linked",
            };
            let mut line = format!(
                "  {:<path_width$}  {:<branch_width$}  {}",
                row.path, branch, kind,
            );
            if row.locked {
                line.push_str(" locked");
            }
            if row.prunable {
                line.push_str(" prunable");
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    out
}

/// Short branch name for display: strip a leading `refs/heads/`.
fn short_branch(refname: &str) -> String {
    refname
        .strip_prefix("refs/heads/")
        .unwrap_or(refname)
        .to_string()
}

/// Human repo name: the basename of the repo's working tree. The repo
/// id carries the common `.git` dir, so strip a trailing `.git` (or
/// `.git/` segment) and take the last path component.
fn repo_display_name(repo: &RepoId) -> String {
    let common = repo.common_dir.trim_end_matches('/');
    let root = common
        .strip_suffix("/.git")
        .or_else(|| common.strip_suffix(".git"))
        .unwrap_or(common)
        .trim_end_matches('/');
    root.rsplit('/')
        .find(|seg| !seg.is_empty())
        .unwrap_or(root)
        .to_string()
}

#[cfg(test)]
#[path = "worktree_tests.rs"]
mod tests;
