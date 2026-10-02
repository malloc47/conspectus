//! Effect executor: runs the effects the reducer emits (ADR 0085 contract 2).

use super::*;
use crate::tui::messages::{LogEntry, LogTarget};

/// Run one pure (no-terminal) effect. Shared between
/// [`execute_effects`] and [`execute_effects_live`] so the two
/// executors stay in lockstep for the non-`Exec` variants.
pub(super) fn execute_pure_effect(app: &mut App, effect: Effect) {
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
        Effect::Exec(_) => {
            // Exec effects need a live terminal; the pure executor
            // silently drops them. Static and snapshot modes gate
            // their translators upstream so `AttachSelected` /
            // `ResumeSelected` never fire there — this branch is a
            // safety net rather than a code path exercised in
            // practice.
        }
        Effect::RunMux(_) => {
            // Mux ops need a live `MuxBackend`; the pure executor
            // silently drops them. Same rationale as `Effect::Exec`
            // above — no static-mode path emits `RunMux` today, so
            // this branch is a safety net.
        }
        Effect::WriteStore(_) => {
            // Some `WriteStore` variants (`PinCreate`,
            // `CommitAliasRename`) chain a tmux rename through the
            // executor, so all store writes flow through
            // `execute_effects_live` where the `MuxBackend`
            // reference lives. The pure executor drops them the
            // same way it drops `RunMux` / `Exec`.
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
        StoreOp::PinCreate(request) => execute_pin_create(app, tmux, request),
        StoreOp::PinEdit(request) => execute_pin_edit(app, request),
        StoreOp::PinRemove(request) => execute_pin_remove(app, request),
        StoreOp::PinBind(request) => execute_pin_bind(app, request),
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

    if let Some(mux_rename) = &plan.mux_native_rename {
        // Default-socket rename only; pin sockets are not
        // propagated here.
        let config = app.config().clone();
        match tmux.rename_session(None, &mux_rename.mux.native_id, &mux_rename.new_name) {
            Ok(crate::discovery::tmux::TmuxRenameOutcome::Renamed) => {}
            Ok(other) => {
                refresh_after_mux_handoff(app, &config);
                app.report(
                    LogEntry::warning(format!("alias updated, tmux rename failed: {other:?}"))
                        .with_target(LogTarget::Mux(mux_rename.mux.clone())),
                );
                return;
            }
            Err(err) => {
                refresh_after_mux_handoff(app, &config);
                app.report(
                    LogEntry::warning(format!("alias updated, tmux rename errored: {err:#}"))
                        .with_target(LogTarget::Mux(mux_rename.mux.clone())),
                );
                return;
            }
        }
    }

    let advisory = live_session_advisory(app, &session_id);
    let config = app.config().clone();
    refresh_after_mux_handoff(app, &config);
    let final_status = match advisory {
        Some(suffix) => format!("{alias_status} · {suffix}"),
        None => alias_status,
    };
    app.report(LogEntry::info(final_status));
}

/// Executor branch for `StoreOp::CommitMuxRename`. Graph-aware
/// cascade: `plan_mux_rename` finds pins whose `mux.name` matches
/// the mux's current bare native id (and socket), and this executor
/// rewrites each of those pin store TOMLs before firing the tmux
/// `rename-session` so pin bindings survive the rename. Default
/// socket only.
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

    // Rewrite each affected pin store first. If any write fails we
    // still attempt the tmux rename — the pin store update is
    // idempotent on the next run and the operator is more likely to
    // want the tmux rename effective than to want it reverted.
    let mut pin_updates_written = 0usize;
    let mut pin_update_failures = Vec::new();
    for update in &plan.pin_mux_name_updates {
        match rewrite_pin_mux_name(update) {
            Ok(changed) => {
                if changed {
                    pin_updates_written += 1;
                }
            }
            Err(err) => {
                pin_update_failures.push(format!("{}: {err}", update.pin_id));
            }
        }
    }

    // Chain the tmux rename (default socket only; see above).
    let bare_current = plan
        .mux_rename
        .mux
        .native_id
        .rsplit_once(':')
        .map_or(plan.mux_rename.mux.native_id.as_str(), |(_, name)| name);
    let (renamed, mux_status) =
        match tmux.rename_session(None, bare_current, &plan.mux_rename.new_name) {
            Ok(crate::discovery::tmux::TmuxRenameOutcome::Renamed) => (
                true,
                format!("renamed mux to `{}`", plan.mux_rename.new_name),
            ),
            Ok(other) => (false, format!("tmux rename returned {other:?}")),
            Err(err) => (false, format!("tmux rename errored: {err:#}")),
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

    let config = app.config().clone();
    refresh_after_mux_handoff(app, &config);
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

/// Load the pin entry by id, rewrite its `mux.name`, and write it
/// back through `upsert_pin_entry`. Returns `Ok(true)` when the
/// store actually changed; `Ok(false)` when the pin was already
/// pointing at the new name (idempotent). Errors from either half
/// bubble up as `String`s so the caller can list them in the
/// aggregate status message.
pub(super) fn rewrite_pin_mux_name(
    update: &crate::rename::PinMuxNameUpdate,
) -> Result<bool, String> {
    let store_paths = [std::path::PathBuf::from(&update.store_path)];
    let loaded = crate::pins::load_pin_entry_by_id(&store_paths, &update.pin_id)
        .map_err(|err| err.to_string())?
        .ok_or_else(|| format!("pin `{}` not found in store", update.pin_id))?;
    let (_path, mut entry) = loaded;
    if entry.mux.name == update.new_mux_name {
        return Ok(false);
    }
    entry.mux.name = update.new_mux_name.clone();
    let outcome =
        crate::pins::upsert_pin_entry(&update.store_path, entry).map_err(|err| err.to_string())?;
    Ok(outcome.changed)
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
            let _ = app.update(Msg::SetMuxPreview { mux, content });
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
) {
    use crate::tui::effect::ExecSpec;
    match spec {
        ExecSpec::AttachMux(target) => {
            let outcome = run_tmux_attach(terminal, &target);
            app.update(Msg::HandoffReturned(target.mux.clone()));
            app.report(attach_return_entry(
                &target_short(&target),
                outcome,
                LogTarget::Mux(target.mux.clone()),
                &target.native_id,
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
        ExecSpec::ViewSession(session_id) => {
            execute_view_session(terminal, app, config, session_id);
        }
        ExecSpec::LaunchPin {
            pin_id,
            attach_target,
        } => {
            execute_launch_pin(terminal, app, config, &pin_id, attach_target.as_ref());
        }
        ExecSpec::MuxNew { name, cwd } => {
            execute_mux_new(terminal, app, config, &name, &cwd);
        }
        ExecSpec::MuxLaunch { request } => {
            execute_mux_launch(terminal, app, config, request);
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
) {
    if let Some(state) = crate::tui::viewer_bridge::build_viewer_state(&session_id) {
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
