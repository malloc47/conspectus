//! `conspectus worktree` subcommand (H-WT-002, ADR 0092).
//!
//! Read-only for now: `worktree list` renders the git worktrees
//! discovery found across the scanned repos. Mutation
//! (`worktree new` / `rm`, delegated to `worktrunk`) lands in
//! H-WT-004.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

use conspectus::model::{GraphNode, GraphSnapshot, RepoId, WorktreeKind};

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
}

impl WorktreeArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            WorktreeCommand::List(args) => args.run(),
        }
    }
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
