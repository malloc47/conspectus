//! Key handlers for overlays that need runtime services.

use super::*;

/// Resolve the current selection to a renameable row and seed the
/// rename overlay. Agent sessions use alias > harness title > empty;
/// pin rows use the pin display name.
pub(super) fn open_rename_overlay(app: &mut App) {
    use crate::tui::rows::{RowId, RowKind};
    let Some(selection) = app.selection().cloned() else {
        app.update(Msg::SetStatus(Some("rename: nothing selected".to_string())));
        return;
    };
    let row = app.tree().rows.iter().find(|row| row.id == selection);
    let (title, initial) = match (selection, row.map(|row| &row.kind)) {
        (
            RowId::AgentSession(crate::model::NodeId::AgentSession(_)),
            Some(RowKind::AgentSession(session_row)),
        ) => (
            " rename session ",
            session_row.display_label().unwrap_or("").to_string(),
        ),
        (RowId::Pin { .. }, Some(RowKind::Pin(pin))) => (" rename pin ", pin.display_name.clone()),
        (RowId::MuxSession(_), Some(RowKind::MuxSession(mux))) => {
            // Seed with the bare tmux name (post-`<socket>:` prefix
            // when a non-default socket is in play) so the operator
            // types what tmux itself will show.
            let bare = mux
                .native_id
                .rsplit_once(':')
                .map_or(mux.native_id.as_str(), |(_, name)| name);
            (" rename mux ", bare.to_string())
        }
        _ => {
            app.update(Msg::SetStatus(Some(
                "rename: select an agent session, mux, or pin row first".to_string(),
            )));
            return;
        }
    };
    let state = crate::tui::widgets::input::TextInputState::new(title, initial);
    app.open_rename_overlay(state);
    app.update(Msg::SetStatus(Some(
        "rename: Enter confirm · Esc cancel".to_string(),
    )));
}

/// Forward `key` to the open rename overlay, then act on the
/// resulting outcome. Confirm runs the lockstep plan (alias write +
/// optional tmux rename) and refreshes; Cancel just closes the
/// overlay.
pub(super) fn handle_rename_overlay_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.rename_overlay_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_rename_overlay();
            app.update(Msg::SetStatus(Some("rename: cancelled".to_string())));
        }
        OverlayOutcome::Commit(msg) => {
            app.close_rename_overlay();
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
    }
}

/// Forward `key` to the open worktree menu, then act on the outcome
/// (mirrors the rename overlay). Commit runs the mutation via the
/// executor and refreshes; Close just dismisses.
pub(super) fn handle_worktree_menu_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.worktree_menu_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_worktree_menu();
            app.update(Msg::SetStatus(Some("worktree: cancelled".to_string())));
        }
        OverlayOutcome::Commit(msg) => {
            app.close_worktree_menu();
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
    }
}

pub(super) fn handle_new_mux_form_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.new_mux_form_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_new_mux_form();
            app.update(Msg::SetStatus(Some("mux: cancelled".to_string())));
        }
        OverlayOutcome::Commit(msg) => {
            app.close_new_mux_form();
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
    }
}

/// Forward a key to the Mux action menu (ADR 0096).
/// Commit pops the menu and dispatches the emitted Msg through the
/// reducer; the reducer's `Msg::OpenNewMuxForm` /
/// `Msg::OpenMuxLaunchForm` arm pushes the target modal seeded from
/// current selection.
pub(super) fn handle_mux_menu_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.mux_menu_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_mux_menu();
            app.update(Msg::SetStatus(Some("mux: cancelled".to_string())));
        }
        OverlayOutcome::Commit(msg) => {
            app.close_mux_menu();
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
    }
}

/// Forward a key to the mux-launch form (ADR 0096).
pub(super) fn handle_mux_launch_form_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.mux_launch_form_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_mux_launch_form();
            app.update(Msg::SetStatus(Some("mux launch: cancelled".to_string())));
        }
        OverlayOutcome::Commit(msg) => {
            app.close_mux_launch_form();
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
    }
}

/// Dispatch a key into the open full-value modal via the shared
/// [`crate::tui::Overlay`] contract (ADR 0085 contract 3). Close /
/// Commit outcomes pop the stack; Consumed leaves the modal open.
pub(super) fn handle_value_modal_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.value_modal_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => app.close_value_modal(),
        OverlayOutcome::Commit(msg) => {
            app.close_value_modal();
            dispatch(app, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch(app, *msg);
        }
    }
}

/// Dispatch a key into the open help overlay via the shared
/// [`crate::tui::Overlay`] contract (ADR 0085 contract 3). Close /
/// Commit outcomes pop the stack; Consumed leaves the overlay
/// open.
/// Dispatch a key into the open Messages overlay (ADR 0105). `y`
/// copies the selected entry's full record here because the
/// clipboard write is a side effect the widget doesn't own.
pub(in crate::tui) fn handle_messages_overlay_key(
    app: &mut App,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    use ratatui::crossterm::event::{KeyCode, KeyModifiers};
    if key.code == KeyCode::Char('y') && !key.modifiers.contains(KeyModifiers::CONTROL) {
        let text = app
            .messages_overlay()
            .and_then(|state| state.selected_entry(app.messages()))
            .map(crate::tui::messages::LogEntry::full_text);
        if let Some(text) = text {
            match crate::tui::clipboard::copy_to_clipboard(&text) {
                Ok(()) => app.post_toast("copied message"),
                Err(err) => app.post_toast(format!("copy failed: {err}")),
            }
        }
        return;
    }
    let outcome = match app.messages_overlay_mut() {
        Some((state, log)) => state.handle(log, key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => app.close_messages_overlay(),
        OverlayOutcome::Commit(msg) => {
            app.close_messages_overlay();
            dispatch(app, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch(app, *msg);
        }
    }
}

pub(in crate::tui) fn handle_help_overlay_key(
    app: &mut App,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.help_overlay_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => app.close_help_overlay(),
        OverlayOutcome::Commit(msg) => {
            app.close_help_overlay();
            dispatch(app, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch(app, *msg);
        }
    }
}

/// Dispatch a key into the open search overlay, refresh its match
/// list from the visible row tree using the configured backend,
/// and act on its outcome (Confirm picks a row, Cancel closes).
pub(in crate::tui) fn handle_search_overlay_key(
    app: &mut App,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::search::items_from_rows;
    use crate::tui::widgets::search::SearchContext;
    use crate::tui::{Overlay, OverlayOutcome};
    // The items own their strings, so building them ends the borrow
    // of `app` before the overlay state is borrowed mutably.
    let items = items_from_rows(app.visible_rows());
    let ctx = SearchContext { items: &items };
    let outcome = match app.search_overlay_mut() {
        Some(state) => state.handle(ctx, key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_search_overlay();
        }
        OverlayOutcome::Commit(msg) => {
            app.close_search_overlay();
            dispatch(app, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch(app, *msg);
        }
    }
}

/// Handle the controls overlay's key event and apply the resulting
/// action to the app, re-deriving the row tree when needed.
pub(in crate::tui) fn handle_controls_overlay_key(
    app: &mut App,
    _config: &RunConfig,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::widgets::controls::ControlsContext;
    use crate::tui::{Overlay, OverlayOutcome};
    // Snapshot the live state into owned copies so the immutable
    // borrow on `app` ends before we re-borrow it mutably to
    // dispatch the key into the overlay.
    let view = app.active_view();
    let grouping = app.grouping();
    let filter_snapshot = app.filter().clone();
    let sort = app.sort();
    let mux_recency = app.mux_recency();
    let preview_wrap = app.preview_wrap();
    let ctx = ControlsContext {
        view,
        grouping,
        filter: &filter_snapshot,
        sort,
        mux_recency,
        preview_wrap,
    };
    let outcome = match app.controls_overlay_mut() {
        Some(state) => state.handle(&ctx, key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_controls_overlay();
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch(app, *msg);
        }
        OverlayOutcome::Commit(msg) => {
            app.close_controls_overlay();
            dispatch(app, *msg);
        }
    }
}

/// Handle the pins overlay's key event and apply the resulting
/// action to the app. Mirrors [`handle_controls_overlay_key`] but
/// dispatches `PinsAction` through the pin-specific write path.
pub(super) fn handle_pins_overlay_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let ctx = app.pins_context();
    let outcome = match app.pins_overlay_mut() {
        Some(state) => state.handle(&ctx, key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_pins_overlay();
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::Commit(msg) => {
            app.close_pins_overlay();
            dispatch_live(terminal, app, config, tmux, *msg);
        }
    }
}

/// Scenario-mode counterpart to [`handle_pins_overlay_key`].
#[cfg(any(test, debug_assertions, feature = "snapshot"))]
pub(super) fn static_handle_pins_overlay_key(
    app: &mut App,
    config: &RunConfig,
    snapshot: &crate::model::GraphSnapshot,
    key: ratatui::crossterm::event::KeyEvent,
) -> Result<()> {
    use crate::tui::{Overlay, OverlayOutcome};
    let ctx = app.pins_context();
    let outcome = match app.pins_overlay_mut() {
        Some(state) => state.handle(&ctx, key),
        None => return Ok(()),
    };
    let _ = (config, snapshot);
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_pins_overlay();
        }
        OverlayOutcome::CommitAndStay(msg) => {
            static_apply_pins_msg(app, *msg)?;
        }
        OverlayOutcome::Commit(msg) => {
            app.close_pins_overlay();
            static_apply_pins_msg(app, *msg)?;
        }
    }
    Ok(())
}

/// Translate a key event into a [`crate::viewer::input::ViewerMsg`],
/// run it through the pure reducer, and put the new state back on
/// `app` — unless the reducer's effect was `Close`, in which case
/// dismiss the modal. Keys that don't map are dropped silently
/// (the modal owns every keystroke while open).
pub(super) fn handle_viewer_overlay_key(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    use crate::viewer::input::ViewerMsg;
    if app.viewer_modal().is_none() {
        return;
    }
    let vmsg = match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Some(ViewerMsg::Close),
        (_, KeyCode::Esc | KeyCode::Char('q')) => Some(ViewerMsg::Close),
        (_, KeyCode::Char('j') | KeyCode::Down) => Some(ViewerMsg::ScrollDown),
        (_, KeyCode::Char('k') | KeyCode::Up) => Some(ViewerMsg::ScrollUp),
        (_, KeyCode::PageDown | KeyCode::Char(' ')) => Some(ViewerMsg::PageDown),
        (_, KeyCode::PageUp) => Some(ViewerMsg::PageUp),
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => Some(ViewerMsg::HalfPageDown),
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => Some(ViewerMsg::HalfPageUp),
        (_, KeyCode::Char('g') | KeyCode::Home) => Some(ViewerMsg::JumpToStart),
        (KeyModifiers::SHIFT | KeyModifiers::NONE, KeyCode::Char('G')) | (_, KeyCode::End) => {
            Some(ViewerMsg::JumpToEnd)
        }
        (_, KeyCode::Char('t')) => Some(ViewerMsg::CycleToolDetail),
        (KeyModifiers::SHIFT | KeyModifiers::NONE, KeyCode::Char('T')) => {
            Some(ViewerMsg::ToggleThinking)
        }
        (KeyModifiers::SHIFT | KeyModifiers::NONE, KeyCode::Char('I')) => {
            Some(ViewerMsg::ToggleAborted)
        }
        (_, KeyCode::Char('?')) => Some(ViewerMsg::ToggleHelp),
        _ => None,
    };
    let Some(vmsg) = vmsg else {
        return;
    };
    // Nested-reducer dispatch: the App reducer's Msg::Viewer arm
    // pops the top viewer state, runs it through
    // viewer::input::reduce, and pushes the new state back
    // (or leaves it popped on ViewerEffect::Close).
    dispatch(app, Msg::Viewer(vmsg));
}
