//! Keymap layer: crossterm event → [`Action`] translation.
//! Pure with respect to terminal state — the
//! runtime holds no direct crossterm dependency beyond calling
//! [`translate`] on incoming events.
//!
//! Split out of `tui::runtime` so `runtime.rs` can shrink toward
//! the ADR 0085 contract 5 target of a thin loop-driver module
//! separate from the keymap. Focus-aware remapping ([`remap_for_focus`])
//! and view cycling ([`cycle_view`]) live here too since they are
//! keymap-adjacent transformations the loop applies before
//! dispatch.
//!
//! Overlay-owned input is not this module's responsibility — the
//! [`crate::tui::modal`] contract routes keys to the top-of-stack
//! overlay before the fallback keymap sees them (see
//! `overlay_key_from_event` in `runtime.rs`).

use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::tui::app::{App, Focus};
use crate::tui::{Msg, View};

/// Semantic action the runtime dispatches once a crossterm event
/// is translated. Overlay-owned keys arrive as `*OverlayKey`
/// variants; everything else flows through
/// `crate::tui::runtime::LoopMode::dispatch`.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "snapshot", allow(dead_code))]
pub enum Action {
    Msg(Box<Msg>),
    Refresh,
    Attach,
    /// Open the rename overlay for the current selection. The
    /// runtime resolves the AgentSession id and seeds the input
    /// buffer with the current alias, harness title, or an empty
    /// string per ADR 0030.
    OpenRename,
    /// Forward a key event into the open rename overlay.
    RenameOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// `w` — open the worktree action menu for the selected node.
    OpenWorktreeMenu,
    /// `X` — open the worktree menu straight into close-down for the
    /// selected node.
    OpenWorktreeCloseDown,
    /// Forward a key event into the open worktree menu.
    WorktreeMenuKey(ratatui::crossterm::event::KeyEvent),
    /// `n` — open the bare tmux new-session form seeded from the
    /// current selection (ADR 0095).
    OpenNewMuxForm,
    /// Forward a key event into the open bare mux form.
    NewMuxFormKey(ratatui::crossterm::event::KeyEvent),
    /// `m` — open the Mux action menu (ADR 0096).
    /// Discharges the ADR 0095 follow-up now that a second bare-mux-
    /// shape verb (mux launch) has landed.
    OpenMuxMenu,
    /// Forward a key event into the open Mux action menu.
    MuxMenuKey(ratatui::crossterm::event::KeyEvent),
    /// Forward a key event into the open mux-launch form.
    MuxLaunchFormKey(ratatui::crossterm::event::KeyEvent),
    /// `Delete` on an unbound/stale pin row. First press arms a
    /// confirmation; second press shells out to `conspectus pin rm`.
    RemovePin,
    /// `b` on a selected ambiguous pin-bound row. Opens the bind
    /// picker when the resolver flagged competing `PinAmbiguous`
    /// candidates; otherwise surfaces a status hint.
    PinBindHint,
    /// `N` opens the pin create form seeded from the current
    /// selection. Direct counterpart to `Pins > create` on `p`.
    OpenPinCreate,
    /// `B` opens the mux-only rebind form for the selected pin.
    /// Refuses with a status hint when no pin row is selected.
    OpenPinRebind,
    /// `A` opens the create form with adopt selected for a live mux
    /// row. Refuses with a status hint on any other row kind.
    OpenPinAdopt,
    /// `L` launches the selected pin via `conspectus pin launch
    /// <id>`. Sibling of `Enter` on a `RowKind::Pin`; refuses with
    /// a status hint when no pin row is selected.
    LaunchPin,
    /// Open the controls overlay (ADR 0031) at its top
    /// section.
    OpenControls,
    /// Forward a key event into the open controls overlay.
    ControlsOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// Open the pins overlay (ADR 0057) at the top of the action
    /// menu. Sibling of `OpenControls`; kept separate so view/filter
    /// and pin CRUD stay one-key-each on `f` and `p`.
    OpenPins,
    /// Forward a key event into the open pins overlay.
    PinsOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// Switch to a specific view (1–5 accelerators).
    SwitchView(View),
    /// Cycle to the next (delta > 0) or previous (delta < 0) view
    /// (the `]` / `[` accelerators).
    CycleView(i32),
    /// Cycle grouping for the active view forward or back. Bound
    /// to `Ctrl-G` because the End binding owns plain `G`.
    CycleGrouping(i32),
    /// Clear every active filter for the visible view (`F`).
    ClearFilters,
    /// Open the `/` search overlay.
    OpenSearch,
    /// Forward a key event into the open search overlay.
    SearchOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// Open the `?` help overlay.
    OpenHelp,
    /// Forward a key event into the open help overlay.
    HelpOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// Open the `o` full-value modal on the active explorer
    /// cursor row, when the row has a truncated value.
    OpenValueModal,
    /// Forward a key event into the open full-value modal.
    ValueModalKey(ratatui::crossterm::event::KeyEvent),
    /// Resume the selected un-muxed agent session in a new terminal
    /// (launches the harness binary in the background).
    Resume,
    /// Open the native full-screen transcript viewer modal for
    /// the selected agent session (ADR 0052).
    /// The widget renders inside the existing terminal — no alt-
    /// screen swap, no child process. Falls through to the
    /// escape-hatch external launch when the harness has no
    /// native parser (kept for `aider` etc.).
    View,
    /// Forward a key event into the open viewer modal.
    ViewerOverlayKey(ratatui::crossterm::event::KeyEvent),
    /// `Enter` on the left pane. The dispatcher resolves
    /// the selected row's default action: attach a mux row, view an
    /// un-muxed session, or expand/collapse a group row. Right-pane
    /// focus is remapped to [`Action::ExplorerEnter`] before this
    /// variant ever reaches the dispatcher.
    DefaultAction,
    /// `Enter` on the right pane. When the
    /// explorer cursor is on a Node-zone field row with a copyable
    /// value, the runtime writes the value to the clipboard via OSC
    /// 52 (ADR 0056) and posts a toast. Otherwise the dispatcher
    /// falls through to [`Msg::ExplorerActivate`] (group expand,
    /// link drill).
    ExplorerEnter,
    /// `i` on any row. When the selected row is an agent
    /// session or mux session, copies the full id to the clipboard
    /// (ADR 0056) and posts a toast. No-op with a status hint
    /// otherwise.
    CopySessionId,
}

/// Step the view enum forward (delta > 0) or back (delta < 0),
/// wrapping. Used by the `]` / `[` accelerator pair.
pub fn cycle_view(view: View, delta: i32) -> View {
    use crate::tui::widgets::controls::VIEW_OPTIONS;
    let idx = VIEW_OPTIONS.iter().position(|v| *v == view).unwrap_or(0);
    VIEW_OPTIONS[crate::tui::cursor::wrap_step(idx, VIEW_OPTIONS.len(), delta)]
}

/// Re-map an action based on which pane currently has focus. Used
/// so that j/k drive whichever pane the operator is looking at —
/// left pane focus keeps them on the row tree; right pane focus
/// routes them into the explorer. Also handles the
/// Enter → ExplorerEnter remap and F →
/// ExplorerToggleFullDetail. Only key-derived actions
/// touch this pass; overlay-owned keys never reach here — the
/// modal stack routes them before the fallback keymap.
///
/// Uppercase J/K continue to scroll the preview regardless of focus,
/// so operators with the left panel focused can still poke the
/// preview without switching panes.
pub fn remap_for_focus(action: Action, focus: Focus) -> Option<Action> {
    if focus != Focus::Right {
        return Some(action);
    }
    match action {
        Action::Msg(boxed) => match *boxed {
            // J/k drive the explorer cursor when the right
            // pane has focus, replacing the prior raw preview-scroll
            // remap. Uppercase J/K still scroll the preview.
            Msg::NavDown => Some(Action::Msg(Box::new(Msg::ExplorerNavDown))),
            Msg::NavUp => Some(Action::Msg(Box::new(Msg::ExplorerNavUp))),
            Msg::PageDown(_) => Some(Action::Msg(Box::new(Msg::ExplorerNavDown))),
            Msg::PageUp(_) => Some(Action::Msg(Box::new(Msg::ExplorerNavUp))),
            // `e` toggles group expansion; on a non-header row the
            // reducer surfaces a status hint.
            Msg::ToggleLinkedDetails => Some(Action::Msg(Box::new(Msg::ExplorerToggleGroup))),
            // `h`/`l`/`←`/`→` are left-tree expand/collapse keys.
            // When the right pane has focus, drop them so they
            // don't reach across panes and mutate the tree the
            // operator is no longer driving. The explorer has no
            // analogous binding in v1; `Enter` drills, `Backspace`
            // pops a hop.
            Msg::ExpandRow | Msg::CollapseRow => None,
            // `g`/`Home` and `G`/`End` should snap the
            // explorer cursor to its first / last row on right-
            // pane focus, the right-pane-equivalent of how those
            // keys jump the left-tree selection. `Tab` /
            // `Msg::CycleFocus` is intentionally left alone — that
            // key is the focus toggle itself and stays useful
            // regardless of which pane currently has focus.
            Msg::Home => Some(Action::Msg(Box::new(Msg::ExplorerHome))),
            Msg::End => Some(Action::Msg(Box::new(Msg::ExplorerEnd))),
            other => Some(Action::Msg(Box::new(other))),
        },
        // Enter on the explorer cursor either
        // copies a Node-zone field value or expands a
        // group header / drills into a link row. The
        // dispatcher inspects the cursor row at action time, so
        // remap to [`Action::ExplorerEnter`] and let the main
        // loop branch with App state in hand. Left-pane Enter
        // (DefaultAction) is dispatched against the selected
        // row's kind separately.
        Action::DefaultAction => Some(Action::ExplorerEnter),
        // `F` toggles the Expanded Node Detail view when the
        // right pane is focused. The same key still clears filters
        // when the left tree has focus (ADR 0031).
        Action::ClearFilters => Some(Action::Msg(Box::new(Msg::ExplorerToggleFullDetail))),
        other => Some(other),
    }
}

/// Map crossterm events to [`Action`]s. Returns `None` for events
/// the v1 shell ignores. Pulled out so tests don't need a terminal.
/// Focus-aware remapping (j/k driving the focused pane) happens in
/// a separate pass via [`remap_for_focus`] so the keymap stays
/// pure with respect to terminal state.
///
/// `viewport_height` is the rendered height of the row tree in
/// rows, used to size PageUp/PageDown jumps. Pass 1 if unknown.
pub fn translate(event: Event, viewport_height: u16) -> Option<Action> {
    // Consult the declarative KEYBINDINGS table first; keys it
    // doesn't hold fall through to the match below.
    if let Event::Key(key) = &event
        && key.kind == KeyEventKind::Press
        && let Some(action) = crate::tui::keybindings::translate_via_table(key.modifiers, key.code)
    {
        return Some(action);
    }
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => match (key.modifiers, key.code) {
            // All single-char parametric guards
            // (r/a/b/v/f/p/[/]//?/l/h/e) + arrow / nav keys
            // (j/k/Down/Up/Right/Left/Enter/Backspace/Home/End/g/G)
            // migrated to `keybindings::KEYBINDINGS` via
            // `KeyMatcher::AnyModExceptCtrl` and `KeyMatcher::AnyMod`.
            // Dispatched via `translate_via_table` above.
            //
            // Remaining hand-matched:
            // - `PageDown` / `PageUp` — need `viewport_height` in
            //   the action payload, which the table's
            //   `fn() -> Action` signature doesn't carry.
            (_, KeyCode::PageDown) => Some(Action::Msg(Box::new(Msg::PageDown(viewport_height)))),
            (_, KeyCode::PageUp) => Some(Action::Msg(Box::new(Msg::PageUp(viewport_height)))),
            // `o` opens the full-value modal on the cursor
            // row. The reducer no-ops gracefully if the cursor isn't
            // on a row with a truncated value.
            (m, KeyCode::Char('o')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::OpenValueModal)
            }
            // `i` copies the selected agent or mux session's
            // full id to the clipboard via OSC 52 (ADR 0056) and
            // posts a toast. The runtime branches on selection kind;
            // a non-session row surfaces a status hint instead.
            (m, KeyCode::Char('i')) if !m.contains(KeyModifiers::CONTROL) => {
                Some(Action::CopySessionId)
            }
            (_, KeyCode::Tab) => Some(Action::Msg(Box::new(Msg::CycleFocus))),
            (_, KeyCode::Char('J')) => Some(Action::Msg(Box::new(Msg::ScrollPreviewBy(1)))),
            (_, KeyCode::Char('K')) => Some(Action::Msg(Box::new(Msg::ScrollPreviewBy(-1)))),
            _ => None,
        },
        _ => None,
    }
}

/// Resolved default action for the left-pane cursor. Pure
/// over [`App`] state so it can be reused by the live and static
/// event loops and snapshot-tested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectedDefault {
    /// Group row — `Enter` expands/collapses.
    ToggleExpand,
    /// Mux row or attachable agent session — `Enter` attaches.
    Attach,
    /// Un-muxed agent session — `Enter` opens the transcript viewer.
    View,
    /// Unbound / stale-mux pin row — `Enter` shells out to
    /// `conspectus pin launch <id>` per ADR 0057.
    LaunchPin,
}

/// Classify what `Enter` on the left pane should do for the
/// currently-selected row. Pure over [`App`] state so both the
/// live-loop dispatch and scenario-mode dispatch produce the same
/// classification and snapshot tests exercise it directly.
pub fn selected_default_action(app: &App) -> SelectedDefault {
    use crate::tui::rows::{MuxIndicator, RowKind};
    let Some(selection) = app.selection() else {
        return SelectedDefault::ToggleExpand;
    };
    let Some(row) = app.tree().rows.iter().find(|r| &r.id == selection) else {
        return SelectedDefault::ToggleExpand;
    };
    if pin_placeholder_row(row) {
        return SelectedDefault::LaunchPin;
    }
    match &row.kind {
        RowKind::Group(_) => SelectedDefault::ToggleExpand,
        RowKind::MuxSession(_) | RowKind::AgentSessionMuxCandidate(_) => SelectedDefault::Attach,
        RowKind::AgentSession(session) => match session.mux_state {
            MuxIndicator::Unmuxed => SelectedDefault::View,
            MuxIndicator::Attached | MuxIndicator::Ambiguous { .. } => SelectedDefault::Attach,
        },
        // PR / Fork / Repo rows: no muxable target and no viewer;
        // fall back to toggle so expandable parents still behave.
        RowKind::Pr(_) | RowKind::Fork(_) | RowKind::Repo(_) => SelectedDefault::ToggleExpand,
        // Unbound / stale-mux pin rows hand off to the launch
        // primitive via a subprocess so the launch
        // logic stays in one place.
        RowKind::Pin(_) => SelectedDefault::LaunchPin,
    }
}

fn pin_placeholder_row(row: &crate::tui::rows::Row) -> bool {
    use crate::model::NodeId;
    matches!(
        &row.kind,
        crate::tui::rows::RowKind::AgentSession(_) | crate::tui::rows::RowKind::MuxSession(_)
    ) && match &row.kind {
        crate::tui::rows::RowKind::AgentSession(session) => {
            matches!(&session.primary_node, NodeId::Pin(_))
        }
        crate::tui::rows::RowKind::MuxSession(mux) => matches!(&mux.primary_node, NodeId::Pin(_)),
        _ => false,
    }
}
