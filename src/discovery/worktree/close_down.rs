//! Close-down orchestration (H-WT-006, ADR 0093).
//!
//! "Close down a stream of work" is the compound gesture that lands or
//! discards a worktree's branch, terminates the mux/agent sessions
//! rooted in it, removes the worktree, and drops the pins declared
//! there. This module holds the provider-neutral planning + execution
//! so the CLI (`worktree close`) and the TUI (close-down action / `X`)
//! run one tested orchestration rather than two drifting copies.
//!
//! Planning ([`plan_close_down`]) is a pure graph query. Execution
//! ([`execute_close_down`]) takes the worktree + mux backends and a
//! [`ProcessSignaller`] as trait objects, so the whole flow is
//! exercised in tests with fakes and never spawns a process.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;

use crate::discovery::tmux::MuxBackend;
use crate::discovery::tmux::teardown::{ProcessSignaller, TeardownReport, teardown_mux_session};
use crate::model::{GraphNode, GraphSnapshot, path_is_ancestor_of};
use crate::pins::remove_pin_entry;

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

/// A pin to drop because its stream is going away.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PinDropTarget {
    pub store_path: String,
    pub id: String,
    pub display_name: String,
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
    pub pins: Vec<PinDropTarget>,
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
    let pins = pins_rooted_in(snapshot, &worktree_root);
    Some(CloseDownPlan {
        worktree_root,
        branch: branch.to_string(),
        repo_root,
        live_labels,
        mux_targets,
        pins,
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

/// Live mux sessions rooted inside `worktree_path` — the ones
/// close-down terminates. Ordered by native id for deterministic
/// output.
pub fn live_mux_teardown_targets(
    snapshot: &GraphSnapshot,
    worktree_path: &str,
) -> Vec<MuxTeardownTarget> {
    let root = Path::new(worktree_path);
    let mut targets = Vec::new();
    for node in &snapshot.nodes {
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
                native_id: mux.native_id.clone(),
                // Default-socket only for v1, matching the rename mux
                // mutation's scope (H-PIN-014 lifts this).
                socket_name: None,
                pane_pid: mux.active_pane_pid,
            });
        }
    }
    targets.sort_by(|a, b| a.native_id.cmp(&b.native_id));
    targets
}

/// Pins whose declared cwd is at or under `worktree_path` — the pins
/// close-down drops. Deduped and ordered for deterministic output.
pub fn pins_rooted_in(snapshot: &GraphSnapshot, worktree_path: &str) -> Vec<PinDropTarget> {
    let root = Path::new(worktree_path);
    let mut pins = Vec::new();
    for node in &snapshot.nodes {
        let GraphNode::Pin(pin) = node else {
            continue;
        };
        if path_is_ancestor_of(root, Path::new(&pin.cwd)) {
            pins.push(PinDropTarget {
                store_path: pin.store_path.clone(),
                id: pin.id.id.clone(),
                display_name: pin.display_name.clone(),
            });
        }
    }
    pins.sort();
    pins.dedup();
    pins
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
    /// Display names of pins successfully dropped.
    pub dropped_pins: Vec<String>,
    /// `(display_name, error)` for pins that couldn't be dropped —
    /// best-effort; a pin-write failure doesn't fail the close.
    pub pin_errors: Vec<(String, String)>,
}

/// Run the close-down sequence for `plan` (ADR 0093):
///
/// 1. Terminate each mux session (graceful `SIGTERM` -> grace -> hard
///    kill) *before* touching the worktree, so we never remove a tree
///    out from under a still-dying process.
/// 2. Land (`discard == false`) or drop (`discard == true`) the branch
///    and remove the worktree. `merge` removes the worktree itself;
///    `discard` removes it directly with `force` (sessions are gone).
/// 3. Drop the plan's pins, best-effort.
///
/// Confirmation is the caller's responsibility (interactive prompt in
/// the CLI; modal in the TUI) — this function assumes the operator has
/// already agreed.
pub fn execute_close_down(
    plan: &CloseDownPlan,
    discard: bool,
    merge_target: Option<String>,
    wt_backend: &dyn WorktreeBackend,
    mux: &dyn MuxBackend,
    signaller: &dyn ProcessSignaller,
    grace: Duration,
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

    // 3. Drop pins (best-effort). Only attempt when the worktree step
    //    succeeded — a failed merge/remove leaves the stream intact, so
    //    the pins should stay too.
    let mut dropped_pins = Vec::new();
    let mut pin_errors = Vec::new();
    if matches!(worktree, WorktreeMutationOutcome::Succeeded { .. }) {
        for pin in &plan.pins {
            match remove_pin_entry(&pin.store_path, &pin.id) {
                Ok(_) => dropped_pins.push(pin.display_name.clone()),
                Err(err) => pin_errors.push((pin.display_name.clone(), err.to_string())),
            }
        }
    }

    Ok(CloseDownReport {
        teardowns,
        landed: !discard,
        worktree,
        dropped_pins,
        pin_errors,
    })
}

#[cfg(test)]
#[path = "close_down_tests.rs"]
mod tests;
