//! Executors for worktree mutations (ADR 0092, ADR 0093).

use super::*;
use crate::tui::messages::LogEntry;

/// Resolve the worktree mutation backend for a TUI-triggered create /
/// remove, or a human-readable reason it's unavailable.
pub(super) fn resolve_worktree_mutation_backend()
-> Result<Box<dyn crate::discovery::worktree::WorktreeBackend>, String> {
    let selection = crate::config::load_from_cwd().config.worktree.backend;
    match crate::discovery::worktree::resolve_mutation_backend(
        selection,
        crate::discovery::worktree::worktrunk_available(),
    ) {
        Ok(Some(backend)) => Ok(backend),
        Ok(None) => Err(
            "no mutation backend (read-only; install worktrunk or set `[worktree] backend`)"
                .to_string(),
        ),
        Err(err) => Err(err.to_string()),
    }
}

pub(super) fn execute_worktree_create(app: &mut App, repo_root: String, branch: String) {
    use crate::discovery::worktree::{WorktreeCreateRequest, WorktreeMutationOutcome};
    let backend = match resolve_worktree_mutation_backend() {
        Ok(backend) => backend,
        Err(reason) => {
            app.report(LogEntry::warning(format!("worktree: {reason}")));
            return;
        }
    };
    let result = backend.create(&WorktreeCreateRequest {
        repo_root: std::path::PathBuf::from(&repo_root),
        branch: branch.clone(),
        base: None,
    });
    let entry = match result {
        Ok(WorktreeMutationOutcome::Succeeded { .. }) => {
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            LogEntry::info(format!("created worktree for branch `{branch}`"))
        }
        Ok(WorktreeMutationOutcome::Unsupported) => {
            LogEntry::warning("worktree: backend cannot create")
        }
        Ok(WorktreeMutationOutcome::Failed { message, .. }) => {
            LogEntry::error(format!("worktree create failed: {message}"))
        }
        Err(err) => LogEntry::error(format!("worktree create failed: {err:#}")),
    };
    app.report(entry);
}

pub(super) fn execute_worktree_remove(
    app: &mut App,
    repo_root: String,
    branch: String,
    force: bool,
) {
    use crate::discovery::worktree::{WorktreeMutationOutcome, WorktreeRemoveRequest};
    let backend = match resolve_worktree_mutation_backend() {
        Ok(backend) => backend,
        Err(reason) => {
            app.report(LogEntry::warning(format!("worktree: {reason}")));
            return;
        }
    };
    let result = backend.remove(&WorktreeRemoveRequest {
        repo_root: std::path::PathBuf::from(&repo_root),
        branch: branch.clone(),
        force,
    });
    let entry = match result {
        Ok(WorktreeMutationOutcome::Succeeded { .. }) => {
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            LogEntry::info(format!("removed worktree for branch `{branch}`"))
        }
        Ok(WorktreeMutationOutcome::Unsupported) => {
            LogEntry::warning("worktree: backend cannot remove")
        }
        Ok(WorktreeMutationOutcome::Failed { message, .. }) => {
            LogEntry::error(format!("worktree remove failed: {message}"))
        }
        Err(err) => LogEntry::error(format!("worktree remove failed: {err:#}")),
    };
    app.report(entry);
}

pub(super) fn execute_worktree_merge(app: &mut App, worktree_root: String, target: Option<String>) {
    use crate::discovery::worktree::{WorktreeMergeRequest, WorktreeMutationOutcome};
    let backend = match resolve_worktree_mutation_backend() {
        Ok(backend) => backend,
        Err(reason) => {
            app.report(LogEntry::warning(format!("worktree: {reason}")));
            return;
        }
    };
    let result = backend.merge(&WorktreeMergeRequest {
        worktree_root: std::path::PathBuf::from(&worktree_root),
        target,
    });
    let entry = match result {
        Ok(WorktreeMutationOutcome::Succeeded { .. }) => {
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            LogEntry::info("merged worktree back and removed it")
        }
        Ok(WorktreeMutationOutcome::Unsupported) => {
            LogEntry::warning("worktree: backend cannot merge")
        }
        Ok(WorktreeMutationOutcome::Failed { message, .. }) => {
            LogEntry::error(format!("worktree merge failed: {message}"))
        }
        Err(err) => LogEntry::error(format!("worktree merge failed: {err:#}")),
    };
    app.report(entry);
}

pub(super) fn execute_worktree_prune(app: &mut App, repo_root: String) {
    use crate::discovery::worktree::{WorktreeMutationOutcome, WorktreePruneRequest};
    let backend = match resolve_worktree_mutation_backend() {
        Ok(backend) => backend,
        Err(reason) => {
            app.report(LogEntry::warning(format!("worktree: {reason}")));
            return;
        }
    };
    let result = backend.prune(&WorktreePruneRequest {
        repo_root: std::path::PathBuf::from(&repo_root),
        dry_run: false,
    });
    let entry = match result {
        Ok(WorktreeMutationOutcome::Succeeded { .. }) => {
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            LogEntry::info("pruned merged worktrees")
        }
        Ok(WorktreeMutationOutcome::Unsupported) => {
            LogEntry::warning("worktree: backend cannot prune")
        }
        Ok(WorktreeMutationOutcome::Failed { message, .. }) => {
            LogEntry::error(format!("worktree prune failed: {message}"))
        }
        Err(err) => LogEntry::error(format!("worktree prune failed: {err:#}")),
    };
    app.report(entry);
}

pub(super) fn execute_worktree_close_down(
    app: &mut App,
    tmux: &dyn MuxBackend,
    repo_root: String,
    branch: String,
    discard: bool,
) {
    use crate::discovery::tmux::teardown::SystemSignaller;
    use crate::discovery::worktree::WorktreeMutationOutcome;
    use crate::discovery::worktree::close_down::{execute_close_down, plan_close_down};

    // Rebuild the plan from the held snapshot so it reflects the graph
    // as of the operator's gesture.
    let plan = if let Some(db) = app.snapshot_handle() {
        plan_close_down(db.snapshot(), std::path::PathBuf::from(&repo_root), &branch)
    } else {
        app.update(Msg::SetStatus(Some(
            "worktree: no graph loaded".to_string(),
        )));
        return;
    };
    let Some(plan) = plan else {
        app.update(Msg::SetStatus(Some(format!(
            "worktree: no worktree for `{branch}` to close down"
        ))));
        return;
    };

    let backend = match resolve_worktree_mutation_backend() {
        Ok(backend) => backend,
        Err(reason) => {
            app.report(LogEntry::warning(format!("worktree: {reason}")));
            return;
        }
    };

    let grace = crate::config::load_from_cwd()
        .config
        .worktree
        .teardown_grace;

    let result = execute_close_down(
        &plan,
        discard,
        None,
        backend.as_ref(),
        tmux,
        &SystemSignaller,
        grace,
    );

    let entry = match result {
        Ok(report) => match &report.worktree {
            WorktreeMutationOutcome::Succeeded { .. } => {
                let config = app.config().clone();
                refresh_after_pin_mutation(app, &config);
                let verb = if report.landed {
                    "merged & closed"
                } else {
                    "closed"
                };
                LogEntry::info(format!(
                    "{verb} `{branch}` — ended {} session(s), dropped {} pin(s)",
                    report.teardowns.len(),
                    report.dropped_pins.len(),
                ))
            }
            WorktreeMutationOutcome::Unsupported => LogEntry::warning(format!(
                "worktree: backend cannot {}",
                if report.landed { "merge" } else { "remove" }
            )),
            WorktreeMutationOutcome::Failed { message, .. } => {
                LogEntry::error(format!("close-down failed: {message}"))
            }
        },
        Err(err) => LogEntry::error(format!("close-down failed: {err:#}")),
    };
    app.report(entry);
}
