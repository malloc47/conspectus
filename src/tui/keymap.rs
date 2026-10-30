//! Keymap layer: crossterm event → [`Action`] translation.
//! Pure with respect to terminal state — the
//! runtime holds no direct crossterm dependency beyond calling
//! [`translate`] on incoming events.
//!
//! Split out of `tui::runtime` so `runtime.rs` can shrink toward
//! the ADR 0085 contract 5 target of a thin loop-driver module
//! separate from the keymap. The keys themselves are the scoped keymap
//! of [`crate::tui::keybindings`]; this module resolves an event against
//! it for the focused pane ([`translate_in`]) and keeps view cycling
//! ([`cycle_view`]), a keymap-adjacent step the loop applies before
//! dispatch.
//!
//! Overlay-owned input is not this module's responsibility — the
//! [`crate::tui::modal`] contract routes keys to the top-of-stack
//! overlay before the fallback keymap sees them (see
//! `overlay_key_from_event` in `runtime.rs`).

use ratatui::crossterm::event::{Event, KeyEventKind};

use crate::tui::app::{App, Focus};
use crate::tui::keybindings::{active_scopes, keymap};
use crate::tui::{Msg, View};

/// The key that opens the explorer cursor row's whole value
/// (`ExplorerKey::Inspect`). The explorer's cut-value hint names it from
/// here, so the hint and the binding cannot drift apart.
pub const INSPECT_KEY: char = 'o';

/// Semantic action the runtime dispatches once a crossterm event
/// is translated. A key an open modal owns arrives as
/// [`Action::ModalKey`]; everything flows through
/// `crate::tui::runtime::LoopMode::dispatch`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "snapshot", allow(dead_code))]
pub enum Action {
    Msg(Box<Msg>),
    /// A key for the overlay on top of the modal stack, which owns every
    /// key while it is open.
    ModalKey(ratatui::crossterm::event::KeyEvent),
    Refresh,
    /// Refresh and ask the forges again instead of reusing their
    /// listings (ADR 0116).
    HardRefresh,
    Attach,
    /// Open the rename overlay for the current selection. The
    /// runtime resolves the AgentSession id and seeds the input
    /// buffer with the current alias, harness title, or an empty
    /// string per ADR 0030.
    OpenRename,
    /// `w` — open the worktree action menu for the selected node.
    OpenWorktreeMenu,
    /// `X` — open the worktree menu straight into close-down for the
    /// selected node.
    OpenWorktreeCloseDown,
    /// `n` — open the bare tmux new-session form seeded from the
    /// current selection (ADR 0095).
    OpenNewMuxForm,
    /// `m` — open the Mux action menu (ADR 0096).
    /// Discharges the ADR 0095 follow-up now that a second bare-mux-
    /// shape verb (mux launch) has landed.
    OpenMuxMenu,
    /// Open the controls overlay (ADR 0031) at its top
    /// section.
    OpenControls,
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
    /// `s` — search what was said in the sessions.
    OpenContentSearch,
    /// Open the `?` help overlay.
    OpenHelp,
    /// Open the `!` Messages overlay (ADR 0105).
    OpenMessages,
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
    /// `Enter` on the left pane. The dispatcher resolves
    /// the selected row's default action: attach a mux row, view an
    /// un-muxed session, or expand/collapse a group row. On the right
    /// pane its table takes `Enter` first (the explorer's `Activate`).
    DefaultAction,
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
    VIEW_OPTIONS[tuikit::scroll::wrap_step(idx, VIEW_OPTIONS.len(), delta)]
}

/// The action of `event` in the keymap with `focus`: the focused pane's
/// binding of the key, else the program's. `None` for a key neither
/// binds and for an event that is not a key press. An open modal owns
/// every key, so the caller asks the modal stack first. Pure, so tests
/// need no terminal.
///
/// The left tree's keys are the left pane's bindings, so a key the right
/// pane's table does not bind (`Alt-j`, `Ctrl-j`) leaves the tree alone
/// with the right pane focused instead of moving it behind the operator.
pub fn translate_in(focus: Focus, event: Event) -> Option<Action> {
    let Event::Key(key) = event else {
        return None;
    };
    if key.kind != KeyEventKind::Press {
        return None;
    }
    keymap()
        .resolve(active_scopes(focus), key)
        .map(|binding| binding.action().action())
}

/// [`translate_in`] with the left pane focused, as the program starts.
pub fn translate(event: Event) -> Option<Action> {
    translate_in(Focus::Left, event)
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
    /// Pane row of the Search view — `Enter` opens its scrollback.
    Scrollback,
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
    // A slot row attaches to its mux when live; the attach resolver
    // explains otherwise.
    if matches!(
        row.id,
        crate::tui::rows::RowId::Pin { .. } | crate::tui::rows::RowId::History { .. }
    ) || crate::tui::actions::selected_record(app).is_some()
    {
        return SelectedDefault::Attach;
    }
    // An entity listed under a pin acts like its own row: a mux
    // attaches, a session attaches when it is in a mux and opens in the
    // viewer when it is not.
    if let crate::tui::rows::RowId::Nested { node, .. } = &row.id {
        return match node {
            crate::model::NodeId::MuxSession(_) => SelectedDefault::Attach,
            crate::model::NodeId::AgentSession(_) => {
                let in_a_mux = app.snapshot_handle().is_some_and(|handle| {
                    crate::tui::actions::preferred_mux_for_session(handle.snapshot(), node)
                        .is_some()
                });
                if in_a_mux {
                    SelectedDefault::Attach
                } else {
                    SelectedDefault::View
                }
            }
            _ => SelectedDefault::ToggleExpand,
        };
    }
    match &row.kind {
        RowKind::Group(_) => SelectedDefault::ToggleExpand,
        RowKind::MuxSession(_) => SelectedDefault::Attach,
        RowKind::AgentSession(session) => match session.mux_state {
            MuxIndicator::Unmuxed => SelectedDefault::View,
            MuxIndicator::Attached | MuxIndicator::Ambiguous { .. } => SelectedDefault::Attach,
        },
        // PR rows: no muxable target and no viewer;
        // fall back to toggle so expandable parents still behave.
        RowKind::Pr(_) => SelectedDefault::ToggleExpand,
        // Nested rows act on their own entity: attach a mux, view a
        // session.
        RowKind::Mixed(_) => SelectedDefault::ToggleExpand,
        // A matching turn opens the viewer at that turn.
        RowKind::Hit(_) => SelectedDefault::View,
        // A matching pane line opens the scrollback at that line.
        RowKind::Pane(_) | RowKind::PaneHit(_) => SelectedDefault::Scrollback,
    }
}
