//! Close-down orchestration (ADR 0093).
//!
//! "Close down a stream of work" is the compound gesture that lands or
//! discards a worktree's branch, terminates the mux/agent sessions
//! rooted in it, removes the worktree, and forgets the history records
//! rooted there. This module holds the provider-neutral planning + execution
//! so the CLI (`worktree close`) and the TUI (close-down action / `X`)
//! run one tested orchestration rather than two drifting copies.
//!
//! Planning ([`plan_close_down`]) is a pure graph query. Execution
//! ([`execute_close_down`]) takes the worktree + mux backends and a
//! [`ProcessSignaller`] as trait objects, so the whole flow is
//! exercised in tests with fakes and never spawns a process.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;

use crate::discovery::tmux::MuxBackend;
use crate::discovery::tmux::teardown::{ProcessSignaller, TeardownReport, teardown_mux_session};
use crate::history::HistoryStore;
use crate::model::{GraphNode, GraphSnapshot, path_is_ancestor_of};

use super::{
    WorktreeBackend, WorktreeMergeRequest, WorktreeMutationOutcome, WorktreeRemoveRequest,
};

/// One mux session to terminate during close-down. `native_id` is the
/// backend kill target (the tmux session name for default-socket v1);
/// `pane_pid` drives the graceful `SIGTERM`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MuxTeardownTarget {
    pub native_id: String,
    pub socket_name: Option<String>,
    pub pane_pid: Option<i64>,
}

/// Everything close-down needs to know about a worktree, resolved from
/// the discovered graph. Built by [`plan_close_down`]; consumed by
/// [`execute_close_down`] and the confirmation UI.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseDownPlan {
    pub worktree_root: String,
    pub branch: String,
    pub repo_root: PathBuf,
    /// Human-readable labels for the live agent/mux sessions inside the
    /// worktree — what the confirmation surfaces.
    pub live_labels: Vec<String>,
    pub mux_targets: Vec<MuxTeardownTarget>,
    /// History records a pin slot targets. Closing a stream forgets
    /// the records rooted in the worktree but never these (ADR 0140).
    pub protected_records: BTreeSet<String>,
}

impl CloseDownPlan {
    /// Whether any live session (agent or mux) would be terminated.
    /// Drives the `live` confirmation policy.
    pub fn has_live(&self) -> bool {
        !self.live_labels.is_empty()
    }
}

/// Resolve the close-down plan for `branch` under `repo_root` from the
/// discovered graph. `None` when no enumerated worktree checks out the
/// branch.
pub fn plan_close_down(
    snapshot: &GraphSnapshot,
    repo_root: PathBuf,
    branch: &str,
) -> Option<CloseDownPlan> {
    let worktree_root = worktree_path_for_branch(snapshot, branch)?;
    let live_labels = live_session_labels(snapshot, &worktree_root);
    let mux_targets = live_mux_teardown_targets(snapshot, &worktree_root);
    let protected_records = pinned_records(snapshot);
    Some(CloseDownPlan {
        worktree_root,
        branch: branch.to_string(),
        repo_root,
        live_labels,
        mux_targets,
        protected_records,
    })
}

/// Path of the worktree checking out `branch` (short name).
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

/// `refs/heads/foo` -> `foo`; leaves already-short names untouched.
fn short_branch(refname: &str) -> String {
    refname
        .strip_prefix("refs/heads/")
        .unwrap_or(refname)
        .to_string()
}

/// Human-readable labels for the live agent/mux sessions rooted inside
/// `worktree_path` (what the confirmation lists).
fn live_session_labels(snapshot: &GraphSnapshot, worktree_path: &str) -> Vec<String> {
    let root = Path::new(worktree_path);
    let mut sessions = Vec::new();
    // Gone muxes run nothing to end (ADR 0140).
    for node in snapshot.live_nodes() {
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

/// Live mux sessions rooted inside `worktree_path` — the ones
/// close-down terminates. Ordered by native id for deterministic
/// output.
pub fn live_mux_teardown_targets(
    snapshot: &GraphSnapshot,
    worktree_path: &str,
) -> Vec<MuxTeardownTarget> {
    let root = Path::new(worktree_path);
    let mut targets = Vec::new();
    for node in snapshot.live_nodes() {
        let GraphNode::MuxSession(mux) = node else {
            continue;
        };
        let cwd = mux
            .active_pane_current_path
            .as_deref()
            .or(mux.cwd.as_deref());
        if let Some(cwd) = cwd
            && path_is_ancestor_of(root, Path::new(cwd))
        {
            targets.push(MuxTeardownTarget {
                native_id: mux.session_name().to_string(),
                socket_name: mux.socket.clone(),
                pane_pid: mux.active_pane_pid,
            });
        }
    }
    targets.sort_by(|a, b| a.native_id.cmp(&b.native_id));
    targets
}

/// The history record ids pin slots point at.
fn pinned_records(snapshot: &GraphSnapshot) -> BTreeSet<String> {
    snapshot
        .nodes
        .iter()
        .filter_map(|node| match node {
            GraphNode::Pin(pin) => pin
                .target
                .as_deref()?
                .strip_prefix("history:")
                .map(str::to_string),
            _ => None,
        })
        .collect()
}

/// Outcome of a close-down run — enough for a caller to render a
/// per-step summary.
#[derive(Clone, Debug)]
pub struct CloseDownReport {
    /// `(native_id, teardown report)` for each terminated mux session,
    /// in plan order.
    pub teardowns: Vec<(String, TeardownReport)>,
    /// `true` when the branch was merged back, `false` when discarded.
    pub landed: bool,
    /// Outcome of the worktree merge/remove step.
    pub worktree: WorktreeMutationOutcome,
    /// How many history records rooted in the worktree were forgotten.
    pub forgotten_records: usize,
    /// Why forgetting failed, if it did. Best-effort: a history failure
    /// doesn't fail the close.
    pub history_error: Option<String>,
}

/// Run the close-down sequence for `plan` (ADR 0093):
///
/// 1. Terminate each mux session (graceful `SIGTERM` -> grace -> hard
///    kill) *before* touching the worktree, so we never remove a tree
///    out from under a still-dying process.
/// 2. Land (`discard == false`) or drop (`discard == true`) the branch
///    and remove the worktree. `merge` removes the worktree itself;
///    `discard` removes it directly with `force` (sessions are gone).
/// 3. Forget the history records rooted in the worktree, except those
///    a pin slot targets, best-effort.
///
/// Confirmation is the caller's responsibility (interactive prompt in
/// the CLI; modal in the TUI) — this function assumes the operator has
/// already agreed.
#[allow(clippy::too_many_arguments)]
pub fn execute_close_down(
    plan: &CloseDownPlan,
    discard: bool,
    merge_target: Option<String>,
    wt_backend: &dyn WorktreeBackend,
    mux: &dyn MuxBackend,
    signaller: &dyn ProcessSignaller,
    grace: Duration,
    history: Option<&HistoryStore>,
) -> Result<CloseDownReport> {
    // 1. Terminate mux sessions first.
    let mut teardowns = Vec::new();
    for target in &plan.mux_targets {
        let report = teardown_mux_session(
            mux,
            signaller,
            target.socket_name.as_deref(),
            &target.native_id,
            target.pane_pid,
            grace,
        )?;
        teardowns.push((target.native_id.clone(), report));
    }

    // 2. Land or drop the branch + remove the worktree.
    let worktree = if discard {
        wt_backend.remove(&WorktreeRemoveRequest {
            repo_root: plan.repo_root.clone(),
            branch: plan.branch.clone(),
            force: true,
        })?
    } else {
        wt_backend.merge(&WorktreeMergeRequest {
            worktree_root: PathBuf::from(&plan.worktree_root),
            target: merge_target,
        })?
    };

    // 3. Forget the stream's history (best-effort). Only when the
    //    worktree step succeeded: a failed merge or remove leaves the
    //    stream intact, so its records stay too.
    let mut forgotten_records = 0;
    let mut history_error = None;
    if let Some(store) = history
        && matches!(worktree, WorktreeMutationOutcome::Succeeded { .. })
    {
        match crate::history::reap::forget_rooted_in(
            store,
            Path::new(&plan.worktree_root),
            &plan.protected_records,
        ) {
            Ok(count) => forgotten_records = count,
            Err(err) => history_error = Some(err.to_string()),
        }
    }

    Ok(CloseDownReport {
        teardowns,
        landed: !discard,
        worktree,
        forgotten_records,
        history_error,
    })
}

#[cfg(test)]
#[path = "close_down_tests.rs"]
mod tests;
