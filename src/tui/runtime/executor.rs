//! Effect executor: runs the effects the reducer emits (ADR 0085 contract 2).

use super::*;
use crate::tui::keybindings::UNDO_KEY;
use crate::tui::messages::{LogEntry, LogTarget};
use crate::tui::undo::Undo;

/// Run one pure (no-terminal) effect. Shared between
/// [`execute_effects`] and [`execute_effects_live`] so the two
/// executors stay in lockstep for the non-`Exec` variants.
pub(super) fn execute_pure_effect(app: &mut App, effect: Effect) {
    // These need a live terminal, mux backend, or store. Static and
    // snapshot modes never offer a gesture that emits them; if one
    // arrives anyway, say so instead of dropping it silently.
    if !pure_executor_runs(&effect) {
        app.post_toast("that isn't available in this mode");
        return;
    }
    match effect {
        Effect::Quit => {
            // `Msg::Quit` already set `App::should_quit`; the event
            // loop reads that flag each iteration. The effect
            // signal is redundant with the state field today and
            // will become the sole quit signal once every quit
            // path routes through here.
        }
        Effect::Toast(label) => app.post_toast(label),
        Effect::Persist => app.persist_state(),
        Effect::SpawnRefresh { force_local } => {
            if force_local {
                let mut cfg = app.config().clone();
                cfg.refresh = true;
                refresh_with_config(app, &cfg);
            } else {
                let cfg = app.config().clone();
                refresh(app, &cfg);
            }
        }
        Effect::Exec(_) | Effect::RunMux(_) | Effect::WriteStore(_) => {
            unreachable!("the live executor runs these")
        }
    }
}

/// Dispatch a [`StoreOp`] against the on-disk TOML store. The only
/// place in the TUI where a reducer-emitted store write touches
/// `.conspectus.toml` (ADR 0085 contract 2 Phase D). Handles the
/// full post-write flow: refresh so the row tree reflects the
/// change, then post a status message summarizing the outcome.
pub(super) fn execute_store_op(
    app: &mut App,
    tmux: &dyn MuxBackend,
    op: crate::tui::effect::StoreOp,
) {
    use crate::tui::effect::StoreOp;
    match op {
        StoreOp::PinRecord { record_id } => {
            execute_pin(app, &crate::pin_ops::Pinnable::Record(record_id));
        }
        StoreOp::PinNode { node } => execute_pin(app, &crate::pin_ops::Pinnable::Node(node)),
        StoreOp::Unpin { slot } => execute_unpin(app, &slot),
        StoreOp::RemovePin { id } => execute_remove_pin(app, &id),
        StoreOp::RestorePin { slot, index } => execute_restore_pin(app, &slot, index),
        StoreOp::RenameSlot { slot, new_name } => execute_rename_slot(app, tmux, &slot, &new_name),
        StoreOp::SavePin { request } => execute_save_pin(app, tmux, &request),
        StoreOp::RetargetSlot { slot, target } => execute_retarget_slot(app, &slot, &target),
        StoreOp::RefreshSlot { slot } => execute_refresh_slot(app, &slot),
        StoreOp::ForgetRecord { record_id } => {
            execute_reap_records(app, std::iter::once(record_id).collect());
        }
        StoreOp::ReapRecords { ids } => execute_reap_records(app, ids.into_iter().collect()),
        StoreOp::CommitAliasRename {
            session_id,
            new_display_name,
        } => execute_commit_alias_rename(app, tmux, session_id, new_display_name),
        StoreOp::CommitMuxRename { mux_id, new_name } => {
            execute_commit_mux_rename(app, tmux, mux_id, new_name);
        }
        StoreOp::WorktreeCreate { repo_root, branch } => {
            execute_worktree_create(app, repo_root, branch);
        }
        StoreOp::WorktreeRemove {
            repo_root,
            branch,
            force,
        } => execute_worktree_remove(app, repo_root, branch, force),
        StoreOp::WorktreeMerge {
            worktree_root,
            target,
        } => execute_worktree_merge(app, worktree_root, target),
        StoreOp::WorktreeCloseDown {
            repo_root,
            branch,
            discard,
        } => execute_worktree_close_down(app, tmux, repo_root, branch, discard),
        StoreOp::WorktreePrune { repo_root } => execute_worktree_prune(app, repo_root),
    }
}

/// Executor branch for `StoreOp::CommitAliasRename`. Mirrors the
/// old `commit_rename` runtime helper: plan the rename against the
/// held snapshot, write the alias entry (or remove it when the
/// operator cleared the field), then chain a tmux rename when the
/// plan carries a native mux rename. Uses `app.config()` for the
/// post-write refresh so a stale config from an earlier snapshot
/// doesn't leak through.
pub(super) fn execute_commit_alias_rename(
    app: &mut App,
    tmux: &dyn MuxBackend,
    session_id: crate::model::AgentSessionId,
    new_display_name: Option<String>,
) {
    let Some(handle) = app.snapshot_handle().cloned() else {
        let _ = app.update(Msg::SetStatus(Some(
            "rename: no graph loaded yet".to_string(),
        )));
        return;
    };
    let snapshot = handle.snapshot();

    let plan =
        match crate::rename::plan_session_rename(snapshot, &session_id, new_display_name, false) {
            Ok(plan) => plan,
            Err(err) => {
                app.report(LogEntry::error(format!("rename failed: {err}")));
                return;
            }
        };

    let endpoint = crate::declared::declared_endpoint_from_node_id(
        &crate::model::NodeId::AgentSession(session_id.clone()),
    );
    let loader = crate::config::ConfigLoader::from_env();
    let store_path = match crate::declared::select_store_for_declaration(
        &endpoint, &endpoint, snapshot, &loader,
    ) {
        Some(selection) => selection.path,
        None => {
            if let Some(path) = loader.user_config_path() {
                path
            } else {
                app.report(LogEntry::error("rename failed: no alias store available"));
                return;
            }
        }
    };

    let alias_outcome = match &plan.agent_alias_write.display_name {
        Some(name) => crate::aliases::upsert_alias_entry(
            &store_path,
            crate::aliases::AliasEntry {
                node: endpoint,
                display_name: name.clone(),
                reason: None,
            },
        )
        .map(|_| format!("renamed: {name}"))
        .map_err(|err| err.to_string()),
        None => crate::aliases::remove_alias_entry(&store_path, &endpoint)
            .map(|_| "rename: cleared alias".to_string())
            .map_err(|err| err.to_string()),
    };

    let alias_status = match alias_outcome {
        Ok(message) => message,
        Err(err) => {
            app.report(LogEntry::error(format!("rename failed: {err}")));
            return;
        }
    };

    if let Some(mux_rename) = &plan.mux_native_rename
        && let Err(reason) = rename_mux_via_backend(tmux, mux_rename)
    {
        app.report(
            LogEntry::warning(format!("alias updated, {reason}"))
                .with_target(LogTarget::Mux(mux_rename.mux.clone())),
        );
        return;
    }

    let advisory = live_session_advisory(app, &session_id);
    let final_status = match advisory {
        Some(suffix) => format!("{alias_status} · {suffix}"),
        None => alias_status,
    };
    app.report(LogEntry::info(final_status));
}

/// Rename `rename`'s session through its mux's own backend: `tmux`,
/// the loop's runner, for tmux sessions and the system runner for any
/// other. `Err` carries the reason for the operator.
pub(super) fn rename_mux_via_backend(
    tmux: &dyn MuxBackend,
    rename: &crate::rename::MuxNativeRename,
) -> Result<(), String> {
    let backend = rename.backend.as_str();
    let mut slot = None;
    let Some(runner) = crate::discovery::mux_runner_for(backend, tmux, &mut slot) else {
        return Err(format!(
            "mux backend `{backend}` has no runner to rename it"
        ));
    };
    match runner.rename_session(rename.socket.as_deref(), &rename.target, &rename.new_name) {
        Ok(crate::discovery::tmux::TmuxRenameOutcome::Renamed) => Ok(()),
        Ok(other) => Err(format!("{backend} rename failed: {other:?}")),
        Err(err) => Err(format!("{backend} rename errored: {err:#}")),
    }
}

/// Executor branch for `StoreOp::CommitMuxRename`. Graph-aware
/// cascade: `plan_mux_rename` finds pin slots that name the mux, and
/// this executor updates each slot's name override before firing the
/// tmux `rename-session` so the slots keep pointing at the mux.
pub(super) fn execute_commit_mux_rename(
    app: &mut App,
    tmux: &dyn MuxBackend,
    mux_id: crate::model::MuxSessionId,
    new_name: String,
) {
    let Some(handle) = app.snapshot_handle().cloned() else {
        let _ = app.update(Msg::SetStatus(Some(
            "mux rename: no graph loaded yet".to_string(),
        )));
        return;
    };
    let snapshot = handle.snapshot();

    let plan = match crate::rename::plan_mux_rename(snapshot, &mux_id, new_name) {
        Ok(plan) => plan,
        Err(err) => {
            app.report(
                LogEntry::error(format!("mux rename failed: {err}"))
                    .with_target(LogTarget::Mux(mux_id.clone())),
            );
            return;
        }
    };

    // Update each affected pin slot's name override first. If a write
    // fails we still attempt the tmux rename: the update is idempotent
    // on the next run and the operator is more likely to want the
    // rename than to want it reverted.
    let mut pin_updates_written = 0usize;
    let mut pin_update_failures = Vec::new();
    if let Some(store) = crate::pin_slots::SlotStore::from_env() {
        for (slot, result) in crate::rename::apply_pin_slot_updates(&store, &plan.pin_slot_updates)
        {
            match result {
                Ok(true) => pin_updates_written += 1,
                Ok(false) => {}
                Err(err) => pin_update_failures.push(format!("{slot}: {err}")),
            }
        }
    }

    let (renamed, mux_status) = match rename_mux_via_backend(tmux, &plan.mux_rename) {
        Ok(()) => (
            true,
            format!("renamed mux to `{}`", plan.mux_rename.new_name),
        ),
        Err(reason) => (false, reason),
    };

    let mut parts = vec![mux_status];
    if pin_updates_written > 0 {
        parts.push(format!(
            "cascaded to {pin_updates_written} pin{}",
            if pin_updates_written == 1 { "" } else { "s" }
        ));
    }
    for failure in &pin_update_failures {
        parts.push(format!("pin update failed: {failure}"));
    }

    let summary = parts.join(" · ");
    let entry = if !renamed {
        LogEntry::error(summary)
    } else if pin_update_failures.is_empty() {
        LogEntry::info(summary)
    } else {
        LogEntry::warning(summary)
    };
    app.report(entry.with_target(LogTarget::Mux(mux_id)));
}

/// Dispatch a [`MuxOp`] against the executor's `MuxBackend`. The
/// only place in the TUI where a reducer-emitted mux op talks to
/// tmux (ADR 0085 contract 2 Phase C).
pub(super) fn execute_mux_op(app: &mut App, tmux: &dyn MuxBackend, op: crate::tui::effect::MuxOp) {
    use crate::tui::effect::MuxOp;
    match op {
        MuxOp::CapturePreview { mux, native_id } => {
            // Capture against the **raw** backend-native session
            // name (e.g. `editor`), not the backend-prefixed graph
            // id (`tmux:editor`) — tmux itself doesn't understand
            // the latter.
            let content = capture_via(tmux, &native_id);
            let _ = app.update(Msg::SetPreview {
                source: crate::tui::preview::PreviewSource::Pane(mux),
                content,
            });
        }
    }
}

/// Dispatch an [`ExecSpec`] against the live terminal. Every branch
/// owns the terminal handoff, waits for the child to exit,
/// re-enters the alt screen if needed, schedules a follow-up
/// refresh, and posts a status message summarizing the outcome.
pub(super) fn execute_exec_spec(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    spec: crate::tui::effect::ExecSpec,
    policy: crate::tui::effect::Lifecycle,
) {
    use crate::tui::effect::ExecSpec;
    match spec {
        ExecSpec::AttachMux(target) => {
            let outcome = run_mux_attach(terminal, &target, None);
            app.update(Msg::HandoffReturned(target.mux.clone()));
            app.report(attach_return_entry(
                &target.label(),
                outcome,
                LogTarget::Mux(target.mux.clone()),
                &target,
                None,
            ));
        }
        ExecSpec::Resume(target) => match &target {
            ResumeTarget::Launch { label, .. } => {
                let entry = if launch_resume(&target) {
                    LogEntry::info(format!("resumed: {label}"))
                } else {
                    LogEntry::error(format!("resume of {label} failed to launch"))
                };
                app.report(entry);
            }
            other => {
                let _ = app.update(Msg::SetStatus(Some(resume_disabled_reason(other))));
            }
        },
        ExecSpec::ViewSession(session_id, open) => {
            execute_view_session(terminal, app, config, session_id, open.as_ref());
        }
        ExecSpec::PaneScrollback { pane, line, text } => {
            open_pane_scrollback(
                app,
                &crate::discovery::tmux::SystemTmux::new(),
                &pane,
                line,
                &text,
            );
        }
        ExecSpec::MuxNew { name, cwd } => {
            execute_mux_new(terminal, app, &name, &cwd, policy);
        }
        ExecSpec::MuxLaunch { request } => {
            execute_mux_launch(terminal, app, request, policy);
        }
        ExecSpec::Rebuild { reference, .. } => {
            execute_rebuild(terminal, app, &reference, policy);
        }
        ExecSpec::ShowRebuildPlan { reference } => {
            execute_show_rebuild_plan(terminal, app, &reference);
        }
    }
}

/// Executor branch for `ExecSpec::PaneScrollback`: capture the pane's
/// scrollback now and open it in the pager on the line the search found.
pub(super) fn open_pane_scrollback(
    app: &mut App,
    tmux: &dyn MuxBackend,
    pane: &crate::content_search::PaneRef,
    line: usize,
    text: &str,
) {
    use crate::discovery::tmux::TmuxCaptureOutcome;
    let label = format!(
        "{} {}.{} scrollback",
        pane.mux.session, pane.window_index, pane.pane_index
    );
    match tmux.pane_history(pane.mux.socket.as_deref(), &pane.pane_id) {
        Ok(TmuxCaptureOutcome::Captured(capture)) => {
            let scrollback = crate::content_search::panes::scrollback_lines(&capture.text);
            app.open_value_modal(
                crate::tui::widgets::value_modal::ValueModalState::scrollback(
                    label,
                    &scrollback.lines,
                    line,
                    text,
                ),
            );
        }
        _ => {
            let _ = app.update(Msg::SetStatus(Some(format!(
                "{} {}.{} could not be read now",
                pane.mux.session, pane.window_index, pane.pane_index
            ))));
        }
    }
}

/// Executor branch for `ExecSpec::ViewSession`. Tries the native
/// viewer first (ADR 0052) — its filesystem read + parser call
/// keep the reducer pure. Falls through to the external-launch
/// escape hatch for harnesses without a native parser (currently
/// `aider`).
pub(super) fn execute_view_session(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    session_id: crate::model::AgentSessionId,
    open: Option<&crate::viewer::state::ViewerOpen>,
) {
    if let Some(state) = crate::tui::viewer_bridge::build_viewer_state_at(&session_id, open) {
        let label = format!("{}:{}", session_id.harness_key, session_id.session_key);
        app.open_viewer_modal(state);
        let _ = app.update(Msg::SetStatus(Some(format!("viewing {label}"))));
        return;
    }
    let target = resolve_viewer_target(&session_id, &PathBinaryProbe);
    match target {
        ViewerTarget::Launch(plan) => {
            let outcome = run_viewer_launch(terminal, &plan);
            refresh(app, config);
            app.report(match outcome {
                ViewerOutcome::Exited => LogEntry::info(format!("viewed: {}", plan.label)),
                ViewerOutcome::Failed(reason) => LogEntry::error(format!("view failed: {reason}")),
            });
        }
        ViewerTarget::Disabled(reason) => {
            let _ = app.update(Msg::SetStatus(Some(viewer_disabled_reason(&reason))));
        }
    }
}

/// The slot store, or an error entry in the log when there is no data
/// home to keep one in.
fn slot_store_or_report(app: &mut App) -> Option<crate::pin_slots::SlotStore> {
    let store = crate::pin_slots::SlotStore::from_env();
    if store.is_none() {
        app.report(LogEntry::error("pin: no data home to store pins in"));
    }
    store
}

/// Record the outcome of a slot write: refresh the tree on success so
/// markers and the Pinned view catch up, and log either way.
fn finish_slot_write(app: &mut App, outcome: Result<String, String>) {
    let entry = match outcome {
        Ok(summary) => LogEntry::info(summary),
        Err(message) => LogEntry::error(message),
    };
    app.report(entry);
}

/// Executor branch for `StoreOp::PinRecord` and `PinNode`: add a slot
/// that points at the entity (the same gesture as `conspectus pin
/// add`). A live mux, worktree, or repo with no history record gets one
/// first, so the history writer comes along.
fn execute_pin(app: &mut App, what: &crate::pin_ops::Pinnable) {
    let Some(store) = slot_store_or_report(app) else {
        return;
    };
    let Some(handle) = app.snapshot_handle().cloned() else {
        return;
    };
    let history = crate::history::HistoryWriter::from_env(app.config().history.clone());
    let (outcome, undo) = pin_with(&store, handle.snapshot(), history.as_ref(), what);
    if let Some(undo) = undo {
        app.update(Msg::UndoRecorded(undo));
    }
    finish_slot_write(app, outcome);
}

/// Pin `what` in `store`; on success the summary and the undo that
/// removes the new pin.
pub(super) fn pin_with(
    store: &crate::pin_slots::SlotStore,
    snapshot: &crate::model::GraphSnapshot,
    history: Option<&crate::history::HistoryWriter>,
    what: &crate::pin_ops::Pinnable,
) -> (Result<String, String>, Option<Undo>) {
    match crate::pin_ops::pin(snapshot, store, history, what, None) {
        Ok(slot) => (
            Ok(format!("pinned as `{}` · {UNDO_KEY} to undo", slot.id)),
            Some(Undo::Unpin { id: slot.id }),
        ),
        Err(err) => (Err(format!("pin failed: {err}")), None),
    }
}

/// Executor branch for `StoreOp::Unpin`: remove the slot. The entity
/// and its history record stay (the record becomes reapable again).
fn execute_unpin(app: &mut App, slot: &str) {
    let Some(store) = slot_store_or_report(app) else {
        return;
    };
    let entity = app.snapshot_handle().and_then(|handle| {
        crate::tui::rows::pin_nodes(handle.snapshot())
            .into_iter()
            .find(|pin| pin.id.id == slot)
            .and_then(|pin| pin.target_node.clone())
    });
    let (outcome, undo) = unpin_with(&store, slot, entity);
    if let Some(undo) = undo {
        app.update(Msg::UndoRecorded(undo));
    }
    finish_slot_write(app, outcome);
}

/// Remove pin `slot` from `store`; on success the summary and the undo
/// that puts it back whole, in its old place in the order.
pub(super) fn unpin_with(
    store: &crate::pin_slots::SlotStore,
    slot: &str,
    entity: Option<crate::model::NodeId>,
) -> (Result<String, String>, Option<Undo>) {
    let before = store
        .load()
        .slots
        .into_iter()
        .find(|existing| existing.id == slot);
    let overrides = before
        .as_ref()
        .map_or(0, |existing| existing.overrides.len());
    match store.remove(slot) {
        Ok(write) if write.changed => {
            let undo = before.map(|removed| Undo::RestorePin {
                index: usize::try_from(removed.order).unwrap_or(usize::MAX),
                slot: Box::new(removed),
                entity,
            });
            let summary = match overrides {
                0 => format!("unpinned `{slot}` · {UNDO_KEY} to undo"),
                1 => format!("unpinned `{slot}` (its 1 override was removed) · {UNDO_KEY} to undo"),
                n => format!(
                    "unpinned `{slot}` (its {n} overrides were removed) · {UNDO_KEY} to undo"
                ),
            };
            (Ok(summary), undo)
        }
        Ok(_) => (Err(format!("unpin: no pin named `{slot}`")), None),
        Err(err) => (Err(format!("unpin failed: {err}")), None),
    }
}

/// Executor branch for `StoreOp::RemovePin`, the undo of a pin.
fn execute_remove_pin(app: &mut App, id: &str) {
    let Some(store) = slot_store_or_report(app) else {
        return;
    };
    finish_slot_write(app, remove_pin_with(&store, id));
}

/// Remove the pin an undo is reversing.
pub(super) fn remove_pin_with(
    store: &crate::pin_slots::SlotStore,
    id: &str,
) -> Result<String, String> {
    match store.remove(id) {
        Ok(write) if write.changed => Ok(format!("removed pin `{id}`")),
        Ok(_) => Err(format!("undo: pin `{id}` is already gone")),
        Err(err) => Err(format!("undo failed: {err}")),
    }
}

/// Executor branch for `StoreOp::RestorePin`, the undo of an unpin.
fn execute_restore_pin(app: &mut App, slot: &crate::model::PinSlot, index: usize) {
    let Some(store) = slot_store_or_report(app) else {
        return;
    };
    let taken_by_a_legacy_pin = app.snapshot_handle().is_some_and(|handle| {
        handle
            .snapshot()
            .pins
            .iter()
            .any(|pin| pin.legacy && pin.id == slot.id)
    });
    finish_slot_write(
        app,
        restore_pin_with(&store, slot, index, taken_by_a_legacy_pin),
    );
}

/// Put an unpinned pin back, whole, in its old place. A name taken
/// since refuses (the entry is dropped, not retried), and a pin whose
/// history record was reaped meanwhile comes back showing as missing.
pub(super) fn restore_pin_with(
    store: &crate::pin_slots::SlotStore,
    slot: &crate::model::PinSlot,
    index: usize,
    taken_by_a_legacy_pin: bool,
) -> Result<String, String> {
    let exists = || format!("undo: a pin named `{}` exists now", slot.id);
    if taken_by_a_legacy_pin {
        return Err(exists());
    }
    match store.restore(slot, index) {
        Ok(_) => Ok(format!("restored pin `{}`", slot.id)),
        Err(crate::pin_slots::SlotError::DuplicateId(_)) => Err(exists()),
        Err(err) => Err(format!("undo failed: {err}")),
    }
}

/// Executor branch for `StoreOp::RenameSlot`: rename the slot, and a
/// live mux slot's tmux session with it, pinning the name override
/// (ADR 0029 lockstep, as `pin rename`).
fn execute_rename_slot(app: &mut App, tmux: &dyn MuxBackend, slot: &str, new_name: &str) {
    let Some(store) = slot_store_or_report(app) else {
        return;
    };
    let Some(handle) = app.snapshot_handle().cloned() else {
        return;
    };
    let snapshot = handle.snapshot();
    let mut notes = Vec::new();
    let live_mux = crate::tui::rows::pin_nodes(snapshot)
        .into_iter()
        .find(|pin| pin.id.id == slot)
        .and_then(
            |pin| match (pin.kind, pin.status, pin.target_node.as_ref()) {
                (
                    crate::model::PinKind::Mux,
                    crate::model::SlotStatus::Live,
                    Some(crate::model::NodeId::MuxSession(mux)),
                ) => Some(mux.clone()),
                _ => None,
            },
        );
    if let Some(mux) = live_mux {
        match crate::rename::plan_mux_rename(snapshot, &mux, new_name.to_string()) {
            Ok(plan) => {
                for (slot, result) in
                    crate::rename::apply_pin_slot_updates(&store, &plan.pin_slot_updates)
                {
                    if let Err(err) = result {
                        notes.push(format!("pin update failed: {slot}: {err}"));
                    }
                }
                match rename_mux_via_backend(tmux, &plan.mux_rename) {
                    Ok(()) => notes.push(format!("renamed the mux to `{new_name}`")),
                    Err(reason) => {
                        finish_slot_write(app, Err(format!("rename failed: {reason}")));
                        return;
                    }
                }
                let name = (
                    "name".to_string(),
                    crate::model::FieldValue::Text(new_name.to_string()),
                );
                if let Err(err) = crate::pin_ops::edit(&store, slot, &[name], &[]) {
                    notes.push(format!("name override not set: {err}"));
                }
            }
            Err(err) => notes.push(format!("the mux keeps its name: {err}")),
        }
    }
    let outcome = crate::pin_ops::rename(&store, slot, new_name)
        .map(|_| {
            let mut parts = vec![format!("renamed pin `{slot}` to `{new_name}`")];
            parts.extend(notes);
            parts.join(" · ")
        })
        .map_err(|err| format!("rename failed: {err}"));
    finish_slot_write(app, outcome);
}

/// Executor branch for `StoreOp::SavePin`: apply the form's overrides,
/// then rename (which renames a live mux's session too).
fn execute_save_pin(
    app: &mut App,
    tmux: &dyn MuxBackend,
    request: &crate::tui::widgets::pin_edit::PinEditRequest,
) {
    let Some(store) = slot_store_or_report(app) else {
        return;
    };
    let slot = request.pin.as_str();
    match (apply_pin_overrides(&store, request), &request.rename) {
        (Err(err), _) => finish_slot_write(app, Err(format!("edit failed: {err}"))),
        (Ok(_), Some(new_name)) => execute_rename_slot(app, tmux, slot, new_name),
        (Ok(changed), None) => finish_slot_write(
            app,
            Ok(if changed {
                format!("saved `{slot}`")
            } else {
                format!("`{slot}` already had that")
            }),
        ),
    }
}

/// The override half of saving the pin edit form: set and drop the
/// requested overrides, returning whether anything changed.
pub(super) fn apply_pin_overrides(
    store: &crate::pin_slots::SlotStore,
    request: &crate::tui::widgets::pin_edit::PinEditRequest,
) -> Result<bool, crate::pin_ops::PinError> {
    if request.set.is_empty() && request.unset.is_empty() {
        return Ok(false);
    }
    crate::pin_ops::edit(store, &request.pin, &request.set, &request.unset)
}

/// Executor branch for `StoreOp::RetargetSlot`.
fn execute_retarget_slot(app: &mut App, slot: &str, target: &str) {
    let Some(store) = slot_store_or_report(app) else {
        return;
    };
    let Some(handle) = app.snapshot_handle().cloned() else {
        return;
    };
    let history = crate::history::HistoryWriter::from_env(app.config().history.clone());
    let outcome = crate::pin_ops::find_pinnable(handle.snapshot(), target)
        .and_then(|what| {
            crate::pin_ops::retarget(handle.snapshot(), &store, history.as_ref(), slot, &what)
        })
        .map(|_| format!("`{slot}` now points at `{target}`"))
        .map_err(|err| format!("retarget failed: {err}"));
    finish_slot_write(app, outcome);
}

/// Executor branch for `StoreOp::RefreshSlot`.
fn execute_refresh_slot(app: &mut App, slot: &str) {
    let Some(store) = slot_store_or_report(app) else {
        return;
    };
    let Some(handle) = app.snapshot_handle().cloned() else {
        return;
    };
    let outcome = crate::pin_ops::refresh_snapshot(handle.snapshot(), &store, slot)
        .map(|changed| {
            if changed {
                format!("refreshed the snapshot of `{slot}`")
            } else {
                format!("the snapshot of `{slot}` was already current")
            }
        })
        .map_err(|err| format!("refresh failed: {err}"));
    finish_slot_write(app, outcome);
}

/// Executor branch for `StoreOp::ForgetRecord` and `ReapRecords`:
/// remove history records. Only records go; nothing touches tmux, git,
/// or the filesystem (ADR 0140).
fn execute_reap_records(app: &mut App, ids: std::collections::BTreeSet<String>) {
    let Some(store) = crate::history::HistoryStore::from_env() else {
        app.report(LogEntry::error(
            "history: no state home to read records from",
        ));
        return;
    };
    let entry = match crate::history::reap::reap(&store, &ids) {
        Ok(removed) => LogEntry::info(format!(
            "forgot {removed} history record{}",
            if removed == 1 { "" } else { "s" }
        )),
        Err(err) => LogEntry::error(format!("history: could not forget records: {err}")),
    };
    app.report(entry);
}

#[cfg(test)]
#[path = "executor_undo_tests.rs"]
mod undo_tests;
