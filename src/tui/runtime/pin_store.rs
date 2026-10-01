//! Executors that write pin stores (ADR 0057).

use super::*;
use crate::tui::messages::LogEntry;

pub(super) fn execute_pin_remove(
    app: &mut App,
    request: crate::tui::widgets::pins::PinRemoveRequest,
) {
    match write_pin_remove(&request) {
        Ok(outcome) => {
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            let message = if outcome.changed {
                format!(
                    "removed pin `{}` from {}",
                    request.id,
                    outcome.path.display()
                )
            } else {
                format!(
                    "pin `{}` was already absent from {}",
                    request.id,
                    outcome.path.display()
                )
            };
            app.report(LogEntry::info(message));
        }
        Err(err) => {
            app.report(LogEntry::error(format!("pin remove failed: {err}")));
        }
    }
}

pub(super) fn execute_pin_bind(app: &mut App, request: crate::tui::widgets::pins::PinBindRequest) {
    let Some(handle) = app.snapshot_handle().cloned() else {
        // Reducer already gated on this — the branch is a safety
        // net for direct executor callers.
        let _ = app.update(Msg::SetStatus(Some(
            "pin bind failed: no graph loaded yet".to_string(),
        )));
        return;
    };
    let snapshot = handle.snapshot();
    match write_pin_bind(&request, snapshot, &crate::config::ConfigLoader::from_env()) {
        Ok(outcome) => {
            let verb = if outcome.changed {
                "bound"
            } else {
                "unchanged"
            };
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            let message = format!(
                "{verb} pin `{}` to session `{}` in {}",
                request.pin_id,
                request.session_key,
                outcome.path.display()
            );
            app.report(LogEntry::info(message));
        }
        Err(err) => {
            app.report(LogEntry::error(format!("pin bind failed: {err}")));
        }
    }
}

pub(super) fn execute_pin_create(
    app: &mut App,
    tmux: &dyn MuxBackend,
    request: crate::tui::widgets::pins::PinCreateRequest,
) {
    match write_pin_create(
        &request,
        &crate::config::ConfigLoader::from_env(),
        &crate::pin_store_registry::PinStoreRegistry::from_env(),
    ) {
        Ok((outcome, entry, store_kind)) => {
            let is_adopt = request.adopt_source_mux_name.is_some();
            let verb = if outcome.changed {
                if outcome.entry_count == 1 {
                    "wrote"
                } else {
                    "updated"
                }
            } else {
                "unchanged"
            };
            let rename_status = apply_pin_adopt_mux_rename(tmux, &request);
            let pin_id = entry.id.clone();
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            let selected = app.select_pin_after_mutation(&pin_id);
            app.post_toast(pin_create_success_toast(is_adopt, &pin_id));
            let mut message = format!(
                "{verb} pin `{}` in {} ({})",
                entry.id,
                outcome.path.display(),
                pin_store_label(store_kind)
            );
            if let Some(rename_status) = rename_status {
                message.push_str("; ");
                message.push_str(&rename_status);
            }
            if !selected {
                message.push_str("; no visible row matched the new pin");
            }
            app.report(LogEntry::info(message));
        }
        Err(err) => {
            app.report(LogEntry::error(format!("pin create failed: {err}")));
        }
    }
}

pub(super) fn execute_pin_edit(app: &mut App, request: crate::tui::widgets::pins::PinEditRequest) {
    match write_pin_edit(&request) {
        Ok(outcome) => {
            let verb = if outcome.changed {
                "saved"
            } else {
                "unchanged"
            };
            let config = app.config().clone();
            refresh_after_pin_mutation(app, &config);
            app.report(LogEntry::info(format!(
                "{verb} pin `{}` in {}",
                request.id,
                outcome.path.display()
            )));
        }
        Err(err) => {
            app.report(LogEntry::error(format!("pin edit failed: {err}")));
        }
    }
}

pub(super) fn pin_create_success_toast(is_adopt: bool, pin_id: &str) -> String {
    if is_adopt {
        format!("pin adopted; mux already running: `{pin_id}`")
    } else {
        format!("pin created, not started: `{pin_id}`")
    }
}

pub(super) fn apply_pin_adopt_mux_rename(
    tmux: &dyn MuxBackend,
    request: &crate::tui::widgets::pins::PinCreateRequest,
) -> Option<String> {
    let source = request.adopt_source_mux_name.as_deref()?;
    if source == request.mux_name {
        return Some(format!("adopted existing mux `{source}`"));
    }
    match tmux.rename_session(request.mux_socket.as_deref(), source, &request.mux_name) {
        Ok(crate::discovery::tmux::TmuxRenameOutcome::Renamed) => Some(format!(
            "renamed adopted mux `{source}` to `{}`",
            request.mux_name
        )),
        Ok(other) => Some(format!(
            "pin written, but adopted mux rename `{source}` -> `{}` failed: {other:?}",
            request.mux_name
        )),
        Err(err) => Some(format!(
            "pin written, but adopted mux rename `{source}` -> `{}` errored: {err}",
            request.mux_name
        )),
    }
}

pub(super) fn write_pin_create(
    request: &crate::tui::widgets::pins::PinCreateRequest,
    loader: &crate::config::ConfigLoader,
    registry: &crate::pin_store_registry::PinStoreRegistry,
) -> Result<(PinWriteOutcome, PinEntry, PinStoreKind)> {
    let cwd = std::path::PathBuf::from(&request.cwd);
    let selection = match request.store {
        crate::tui::widgets::pins::PinCreateStore::Auto
        | crate::tui::widgets::pins::PinCreateStore::Project => {
            crate::pins::select_store_for_pin(&cwd, loader)?
        }
        crate::tui::widgets::pins::PinCreateStore::User => crate::pins::user_pin_store(loader)?,
    };
    let entry = PinEntry {
        id: request.id.clone(),
        display_name: request.display_name.clone(),
        harness: request.harness.clone(),
        cwd: request.cwd.clone(),
        mux: PinMux {
            backend: TMUX_MUX_BACKEND.to_string(),
            name: request.mux_name.clone(),
            socket_name: request.mux_socket.clone(),
        },
        launch: if request.launch_argv.is_empty() {
            None
        } else {
            Some(PinLaunch {
                argv: request.launch_argv.clone(),
            })
        },
        worktree: request
            .worktree_branch
            .clone()
            .map(|branch| crate::pins::PinWorktree { branch }),
        reason: None,
    };
    let outcome = crate::pins::upsert_pin_entry(&selection.path, entry.clone())?;
    // Record the project store so a pin created in a repo outside the
    // scan root stays visible on later discovery cycles.
    // Best-effort: a failed cache write must never
    // fail the pin create itself. `record` no-ops for the user-scope
    // store.
    let _ = registry.record(&selection.path);
    Ok((outcome, entry, selection.kind))
}

pub(super) fn pin_store_label(kind: PinStoreKind) -> &'static str {
    match kind {
        PinStoreKind::Project => "project",
        PinStoreKind::User => "user",
    }
}

pub(super) fn write_pin_bind(
    request: &crate::tui::widgets::pins::PinBindRequest,
    snapshot: &crate::model::GraphSnapshot,
    loader: &crate::config::ConfigLoader,
) -> Result<crate::declared::DeclaredWriteOutcome> {
    let Some(pin) = snapshot.pins.iter().find(|pin| pin.id == request.pin_id) else {
        bail!("no pin `{}` in current graph", request.pin_id);
    };
    let target = snapshot
        .nodes
        .iter()
        .find_map(|node| match node {
            crate::model::GraphNode::AgentSession(session)
                if session.id.harness_key == pin.harness
                    && session.id.session_key == request.session_key =>
            {
                Some(session.id.clone())
            }
            _ => None,
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no `{}` agent session with session_key `{}` in current graph",
                pin.harness,
                request.session_key
            )
        })?;

    let source = crate::declared::DeclaredEndpoint::AgentSession {
        harness_key: target.harness_key.clone(),
        state_scope: target.state_scope.clone(),
        session_key: target.session_key,
    };
    let target_endpoint = crate::declared::DeclaredEndpoint::MuxSession {
        native_id: pin.mux.native_id(),
    };
    let link = crate::declared::DeclaredLink {
        id: format!("pin:{}:bound", pin.id),
        relation: crate::model::RelationKind::LinkedToMux,
        state: crate::declared::DeclaredLinkState::Active,
        source: source.clone(),
        target: target_endpoint.clone(),
        reason: None,
        overridden_by: None,
        label: Some(format!("pin:{}", pin.id)),
    };
    let path =
        crate::declared::select_store_for_declaration(&source, &target_endpoint, snapshot, loader)
            .map(|selection| selection.path)
            .or_else(|| loader.user_config_path())
            .ok_or_else(|| anyhow::anyhow!("no declared-link store available for pin bind"))?;
    Ok(crate::declared::upsert_declared_link(&path, link)?)
}

pub(super) fn write_pin_edit(
    request: &crate::tui::widgets::pins::PinEditRequest,
) -> Result<PinWriteOutcome> {
    let path = std::path::PathBuf::from(&request.store_path);
    preflight_pin_edit(request, &path)?;
    // The edit form doesn't surface the worktree block (ADR 0094), so
    // preserve the edited pin's existing worktree intent from disk
    // rather than dropping it on save.
    let worktree = existing_pin_worktree(&path, &request.original_id);
    if request.original_id != request.id {
        crate::pins::remove_pin_entry(&path, &request.original_id)?;
    }
    let entry = PinEntry {
        id: request.id.clone(),
        display_name: request.display_name.clone(),
        harness: request.harness.clone(),
        cwd: request.cwd.clone(),
        mux: PinMux {
            backend: TMUX_MUX_BACKEND.to_string(),
            name: request.mux_name.clone(),
            socket_name: request.mux_socket.clone(),
        },
        launch: if request.launch_argv.is_empty() {
            None
        } else {
            Some(PinLaunch {
                argv: request.launch_argv.clone(),
            })
        },
        worktree,
        reason: None,
    };
    Ok(crate::pins::upsert_pin_entry(&path, entry)?)
}

/// Read the worktree block of the pin `id` in `path` (ADR 0094), so an
/// edit/rebind can round-trip it without the form having to model it.
/// Best-effort: a read/parse miss yields `None`.
pub(super) fn existing_pin_worktree(
    path: &std::path::Path,
    id: &str,
) -> Option<crate::pins::PinWorktree> {
    let text = std::fs::read_to_string(path).ok()?;
    let document = crate::pins::parse_pins_document(&text).ok()?;
    document
        .entries()
        .iter()
        .find(|entry| entry.id == id)
        .and_then(|entry| entry.worktree.clone())
}

pub(super) fn preflight_pin_edit(
    request: &crate::tui::widgets::pins::PinEditRequest,
    path: &std::path::Path,
) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let document = crate::pins::parse_pins_document(&text)?;
    let mut found_original = false;
    let requested_socket = request.mux_socket.as_deref().unwrap_or("default");
    for entry in document.entries() {
        if entry.id == request.original_id {
            found_original = true;
            continue;
        }
        if entry.id == request.id {
            bail!(
                "pin id `{}` already exists in {}",
                request.id,
                path.display()
            );
        }
        let entry_socket = entry.mux.socket_name.as_deref().unwrap_or("default");
        if entry.mux.backend == TMUX_MUX_BACKEND
            && entry.mux.name == request.mux_name
            && entry_socket == requested_socket
        {
            bail!(
                "mux `{}` is already used by pin `{}` in {}",
                entry.mux.native_id(),
                entry.id,
                path.display()
            );
        }
    }
    if !found_original {
        bail!(
            "pin `{}` was not found in {}",
            request.original_id,
            path.display()
        );
    }
    Ok(())
}

pub(super) fn write_pin_remove(
    request: &crate::tui::widgets::pins::PinRemoveRequest,
) -> Result<PinWriteOutcome> {
    crate::pins::remove_pin_entry(&request.store_path, &request.id).map_err(Into::into)
}
