//! Keys for the modal on top of the stack, and the rename opener.
//!
//! An open modal owns every key. [`modal_key`] gives it the key through
//! the stack (ADR 0161) and does what it asked: closes it for a close or a
//! commit, then hands what it committed, or what it sent while staying
//! open, to the `send` callback. The live loop sends on the live
//! executor, the fixture and scenario TUI on the pure one (which says so
//! when a message needs a terminal, a mux, or a store), and the snapshot
//! driver on its own. So a menu, prompt, form, or viewer moves, edits, and
//! closes the same way in every mode.

use super::*;

/// One key for the modal on top: route it, close the modal for a close
/// or a commit (closing says nothing; see [`App::modal_closed`]), and
/// send what it committed or sent while staying open. Does nothing when
/// no modal is open.
pub(in crate::tui) fn modal_key(
    app: &mut App,
    key: ratatui::crossterm::event::KeyEvent,
    send: &mut dyn FnMut(&mut App, Msg),
) {
    let Some(settled) = app.press_modal_key(key) else {
        return;
    };
    if let Some(closed) = settled.closed
        && let Some(first) = app.modal_closed(closed, settled.message.as_ref())
    {
        send(app, first);
    }
    if let Some(msg) = settled.message {
        send(app, msg);
    }
}

/// [`modal_key`] with what the modal sends run on the pure executor, as
/// the fixture TUI runs it: for tests that drive a modal through the
/// stack.
#[cfg(test)]
pub(in crate::tui) fn modal_key_pure(app: &mut App, key: ratatui::crossterm::event::KeyEvent) {
    modal_key(app, key, &mut dispatch);
}

/// Resolve the current selection to a renameable row and seed the
/// rename overlay. Agent sessions use alias > harness title > empty;
/// pin rows use the pin display name.
pub(in crate::tui) fn open_rename_overlay(app: &mut App) {
    use crate::model::NodeId;
    let subject = crate::tui::actions::action_subject(app);
    if let Some(pin) = subject.pin {
        // A pin has its own prompt, which renames a live mux with it.
        app.update(Msg::OpenSlotRename(pin));
        return;
    }
    let Some(node) = subject.node else {
        app.update(Msg::SetStatus(Some(Feedback::warning(
            "rename: select an agent session or mux first",
        ))));
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
            app.update(Msg::SetStatus(Some(Feedback::warning(
                "rename: that mux is stopped; rebuild it first (Enter, or . for the menu)",
            ))));
            return;
        }
        _ => {
            app.update(Msg::SetStatus(Some(Feedback::warning(
                "rename: select an agent session or mux first",
            ))));
            return;
        }
    };
    let state = crate::tui::widgets::input::TextInputState::new(title, initial);
    app.open_rename_overlay(state);
    app.set_rename_node(node);
    app.update(Msg::SetStatus(Some(Feedback::info(format!(
        "rename: {}",
        tuikit::keys::footer(tuikit::keys::TEXT_INPUT_KEYS)
    )))));
}
