//! Key handlers for overlays that need runtime services.

use super::*;

/// Resolve the current selection to a renameable row and seed the
/// rename overlay. Agent sessions use alias > harness title > empty;
/// pin rows use the pin display name.
pub(super) fn open_rename_overlay(app: &mut App) {
    use crate::model::NodeId;
    let subject = crate::tui::actions::action_subject(app);
    if let Some(pin) = subject.pin {
        // A pin has its own prompt, which renames a live mux with it.
        app.update(Msg::OpenSlotRename(pin));
        return;
    }
    let Some(node) = subject.node else {
        app.update(Msg::SetStatus(Some(
            "rename: select an agent session or mux first".to_string(),
        )));
        return;
    };
    let Some(handle) = app.snapshot_handle().cloned() else {
        return;
    };
    let snapshot = handle.snapshot();
    let (title, initial) = match &node {
        NodeId::AgentSession(id) => {
            let alias = snapshot.aliases.get(&node).map(str::to_string);
            let title = snapshot.nodes.iter().find_map(|n| match n {
                crate::model::GraphNode::AgentSession(s) if &s.id == id => s.title.clone(),
                _ => None,
            });
            (" rename session ", alias.or(title).unwrap_or_default())
        }
        NodeId::MuxSession(mux) if snapshot.is_live(&node) => {
            // Seed with the bare tmux name (post-`<socket>:` prefix
            // when a non-default socket is in play) so the operator
            // types what tmux itself will show.
            let bare = mux
                .native_id
                .rsplit_once(':')
                .map_or(mux.native_id.as_str(), |(_, name)| name);
            (" rename mux ", bare.to_string())
        }
        NodeId::MuxSession(_) => {
            app.update(Msg::SetStatus(Some(
                "rename: that mux is stopped; rebuild it first (Enter, or . for the menu)"
                    .to_string(),
            )));
            return;
        }
        _ => {
            app.update(Msg::SetStatus(Some(
                "rename: select an agent session or mux first".to_string(),
            )));
            return;
        }
    };
    let state = crate::tui::widgets::input::TextInputState::new(title, initial);
    app.open_rename_overlay(state);
    app.set_rename_node(node);
    app.update(Msg::SetStatus(Some(format!(
        "rename: {}",
        crate::tui::keybindings::overlay::footer(crate::tui::keybindings::overlay::TEXT_INPUT_KEYS)
    ))));
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

/// Forward a key to the row action menu. A pick pops the menu and
/// dispatches its message; the reducer does the rest.
pub(super) fn handle_row_menu_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.row_menu_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => app.close_row_menu(),
        OverlayOutcome::Commit(msg) => {
            app.close_row_menu();
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
    }
}

/// Forward a key to the Clean up modal. Confirming pops it and
/// dispatches the reap of the selected records.
pub(super) fn handle_clean_up_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.clean_up_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => app.close_clean_up(),
        OverlayOutcome::Commit(msg) => {
            app.close_clean_up();
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
    }
}

/// Forward a key to the picker. Choosing an item pops it and
/// dispatches what the item commits.
pub(super) fn handle_picker_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.picker_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => app.close_picker(),
        OverlayOutcome::Commit(msg) => {
            app.close_picker();
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
    use crate::tui::keybindings::overlay::{PagerKey, VALUE_MODAL_KEYS, resolve};
    use crate::tui::{Overlay, OverlayOutcome};
    // `y` copies; the clipboard write is a side effect the widget
    // doesn't own.
    if resolve(VALUE_MODAL_KEYS, key) == Some(PagerKey::Copy) {
        let copied = app.value_modal().map(|state| {
            let (text, what) = state.copy_payload();
            (text.to_string(), what)
        });
        if let Some((text, what)) = copied {
            match crate::tui::clipboard::copy_to_clipboard(&text) {
                Ok(()) => app.post_toast(format!("Copied {what}")),
                Err(err) => app.post_toast(format!("copy failed: {err}")),
            }
        }
        return;
    }
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
    use crate::tui::keybindings::overlay::{MESSAGES_KEYS, MessagesKey, resolve};
    use crate::tui::{Overlay, OverlayOutcome};
    if resolve(MESSAGES_KEYS, key) == Some(MessagesKey::Copy) {
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
///
/// Messages the overlay sends run through the pure executor, which is
/// right where there is no terminal; the live loop uses
/// [`handle_search_overlay_key_with`] so a hit that opens the viewer
/// can run its effect.
#[cfg(any(test, debug_assertions, feature = "snapshot"))]
pub(in crate::tui) fn handle_search_overlay_key(
    app: &mut App,
    key: ratatui::crossterm::event::KeyEvent,
) {
    handle_search_overlay_key_with(app, key, &mut |app, msg| dispatch(app, msg));
}

/// [`handle_search_overlay_key`] with the executor the caller chooses
/// for the messages the overlay sends.
pub(in crate::tui) fn handle_search_overlay_key_with(
    app: &mut App,
    key: ratatui::crossterm::event::KeyEvent,
    send: &mut dyn FnMut(&mut App, Msg),
) {
    use crate::tui::search::items_from_rows;
    use crate::tui::widgets::search::{SearchContext, SearchScope};
    use crate::tui::{Overlay, OverlayOutcome};
    // The items own their strings, so building them ends the borrow
    // of `app` before the overlay state is borrowed mutably.
    let items = items_from_rows(app.visible_rows());
    let contents = if app
        .search_overlay()
        .is_some_and(|state| state.scope() == SearchScope::Contents)
    {
        app.content_view().rows
    } else {
        Vec::new()
    };
    let ctx = SearchContext {
        items: &items,
        content: &contents,
    };
    let outcome = match app.search_overlay_mut() {
        Some(state) => state.handle(ctx, key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_search_overlay();
            send(app, Msg::DiscardContentPreview);
        }
        OverlayOutcome::Commit(msg) => {
            app.close_search_overlay();
            // Keeping the search is the only way out that keeps it.
            if !matches!(*msg, Msg::ApplyContentSearch { .. }) {
                send(app, Msg::DiscardContentPreview);
            }
            send(app, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            send(app, *msg);
        }
    }
}

/// Handle the controls overlay's key event and apply the resulting
/// action to the app, re-deriving the row tree when needed.
pub(in crate::tui) fn handle_controls_overlay_key(
    app: &mut App,
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
    let session_preview = app.session_preview();
    let content_snapshot = app
        .content()
        .map(|content| (content.text.clone(), content.status, content.hits().len()));
    let search_kinds = app.config().search.kinds;
    let ctx = ControlsContext {
        view,
        grouping,
        filter: &filter_snapshot,
        sort,
        mux_recency,
        preview_wrap,
        session_preview,
        content: content_snapshot
            .as_ref()
            .map(|(text, status, found)| (text.as_str(), *status, *found)),
        search_kinds,
        content_group: app
            .content()
            .map_or_else(|| app.config().search.group, |content| content.group),
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

/// Translate a key event into a [`crate::viewer::input::ViewerMsg`],
/// run it through the pure reducer, and put the new state back on
/// `app` — unless the reducer's effect was `Close`, in which case
/// dismiss the modal. Keys that don't map are dropped silently
/// (the modal owns every keystroke while open).
pub(in crate::tui) fn handle_viewer_overlay_key(
    app: &mut App,
    key: ratatui::crossterm::event::KeyEvent,
) {
    if app.viewer_modal().is_none() {
        return;
    }
    // While the find prompt is open it owns the keys: bound keys edit
    // or leave it, and anything else types.
    if app
        .viewer_modal()
        .is_some_and(|state| state.find.prompt.is_some())
    {
        use crate::tui::keybindings::overlay::{VIEWER_FIND_KEYS, resolve, typed_char};
        let fmsg = resolve(VIEWER_FIND_KEYS, key)
            .or_else(|| typed_char(key).map(crate::viewer::find::ViewerFindMsg::Insert));
        if let Some(fmsg) = fmsg {
            dispatch(app, Msg::ViewerFind(fmsg));
        }
        return;
    }
    let vmsg = crate::tui::keybindings::overlay::resolve(
        crate::tui::keybindings::overlay::VIEWER_KEYS,
        key,
    );
    let Some(vmsg) = vmsg else {
        return;
    };
    // Nested-reducer dispatch: the App reducer's Msg::Viewer arm
    // pops the top viewer state, runs it through
    // viewer::input::reduce, and pushes the new state back
    // (or leaves it popped on ViewerEffect::Close).
    dispatch(app, Msg::Viewer(vmsg));
}

pub(super) fn handle_pin_edit_form_key(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    config: &RunConfig,
    tmux: &dyn MuxBackend,
    key: ratatui::crossterm::event::KeyEvent,
) {
    use crate::tui::{Overlay, OverlayOutcome};
    let outcome = match app.pin_edit_form_mut() {
        Some(state) => state.handle((), key),
        None => return,
    };
    match outcome {
        OverlayOutcome::Consumed => {}
        OverlayOutcome::Close => {
            app.close_pin_edit_form();
            app.update(Msg::SetStatus(Some("pin edit: cancelled".to_string())));
        }
        OverlayOutcome::Commit(msg) => {
            app.close_pin_edit_form();
            dispatch_live(terminal, app, config, tmux, *msg);
        }
        OverlayOutcome::CommitAndStay(msg) => {
            dispatch_live(terminal, app, config, tmux, *msg);
        }
    }
}
