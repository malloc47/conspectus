//! Modal stack contract (ADR 0085 contract 3).
//!
//! Every open overlay lives on `App::modal_stack: Vec<Modal>`.
//! Input routes to the top of the stack first, `draw` renders the
//! stack in order (bottom-to-top), and `Esc` / commit outcomes pop
//! the top entry. Each overlay implements the [`Overlay`] trait,
//! so adding a new modal becomes a state struct + one `Modal`
//! variant + one trait impl — the keymap, event loop, and draw
//! pipeline acquire zero new branches.
//!
//! Every `Modal` variant implements [`Overlay`]. The trait carries
//! an associated `Ctx<'a>` type so overlays that need live App
//! state (Controls read view/grouping/filter/sort; Pins read a
//! `PinsContext`; Search reads the current row items) can declare
//! it in one place instead of via a specialized runtime handler.
//! Context-free overlays (Help / ValueModal / Viewer / Rename) set
//! `type Ctx<'a> = ()`. The runtime's stack dispatcher matches per
//! `Modal` variant, builds the per-widget context, and calls
//! `overlay.handle(ctx, key)`.

use crate::tui::Msg;

/// A modal surface currently open on the stack. The variant carries
/// the overlay's live state; the [`Overlay`] impl handles input and
/// rendering.
///
/// Every variant here now implements [`Overlay`]. Adding a new one
/// is a state struct + one `Modal` variant + one trait impl; the
/// dispatcher is a single match arm that builds the widget's
/// context and calls the trait method.
#[derive(Debug, Clone)]
pub enum Modal {
    /// `?` help overlay. Context-free.
    Help(crate::tui::widgets::help::HelpOverlayState),
    /// Controls overlay (ADR 0031). Reads a
    /// [`crate::tui::widgets::controls::ControlsContext`] on every
    /// event so view / grouping / filter / sort surface fresh
    /// values as the operator toggles them.
    Controls(crate::tui::widgets::controls::ControlsOverlayState),
    /// Pins overlay (ADR 0057). Reads a
    /// [`crate::tui::widgets::pins::PinsContext`] on every event
    /// so the "adopt selected mux" flow sees the current
    /// selection.
    Pins(crate::tui::widgets::pins::PinsOverlayState),
    /// Rename overlay (ADR 0029 / ADR 0030). Wraps the generic
    /// text-input widget with a [`Msg::CommitRename`] mapper so
    /// the trait's uniform `Commit(Msg)` outcome carries the
    /// specific rename intent.
    Rename(RenameOverlayState),
    /// `/` search overlay. Reads the current visible-row
    /// items as its context and emits a
    /// [`Msg::SelectRow`] on Confirm.
    Search(crate::tui::widgets::search::SearchOverlayState),
    /// `o` full-value modal. Context-free.
    ValueModal(crate::tui::widgets::value_modal::ValueModalState),
    /// `!` Messages overlay (ADR 0105). Reads the App's
    /// [`crate::tui::messages::MessageLog`] as its context.
    Messages(crate::tui::widgets::messages::MessagesOverlayState),
    /// Full-screen transcript viewer modal (ADR 0052). Uses the nested-reducer composition described
    /// in ADR 0085 contract 3: [`crate::tui::Msg::Viewer`]
    /// wraps a `crate::viewer::input::ViewerMsg` and the App
    /// reducer's arm delegates to
    /// `crate::viewer::input::reduce`. The widget's `Close`
    /// effect pops the modal; every other effect leaves it on
    /// the stack.
    Viewer(crate::viewer::state::ViewerState),
    /// `w` worktree action menu. Context-free — it
    /// captures the selected node's worktree facts + guard at open
    /// time and drives an internal list → branch-input / confirm
    /// state machine.
    WorktreeMenu(Box<crate::tui::widgets::worktree_menu::WorktreeMenuState>),
    /// `n` bare tmux new-session form (ADR 0095).
    /// Context-free two-field form (name + cwd) that emits
    /// [`Msg::CommitMuxNew`].
    NewMux(crate::tui::widgets::new_mux::NewMuxFormState),
    /// `m` mux action menu (ADR 0096). Fronts the
    /// mux-specific verbs; each entry commits a Msg that opens the
    /// corresponding target overlay.
    MuxMenu(crate::tui::widgets::mux_menu::MuxMenuState),
    /// Mux-launch form — ephemeral harness in a fresh mux, no pin
    /// (ADR 0096). Commits
    /// [`Msg::CommitMuxLaunch`]. Boxed to keep the `Modal` enum
    /// discriminant small — the launch form carries eight
    /// `TextInputState` fields.
    MuxLaunch(Box<crate::tui::widgets::mux_launch::MuxLaunchFormState>),
}

/// What an overlay wants the runtime to do after a single key
/// event.
///
/// `Commit` and `CommitAndStay` box their [`Msg`] because the
/// message enum is large (over 256 bytes for `Msg::SetData` and
/// friends) and the vast majority of `OverlayOutcome` values are
/// `Consumed` / `Close`; paying an indirection per commit is
/// cheaper than fattening every stack-owned outcome.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayOutcome {
    /// Overlay handled the key — nothing else to do. The stack
    /// stays as-is.
    Consumed,
    /// Overlay committed a value. The runtime pops the overlay,
    /// then dispatches the [`Msg`] through the reducer + executor.
    Commit(Box<Msg>),
    /// Overlay committed a value but wants to stay open. The
    /// runtime dispatches the [`Msg`] and leaves the overlay on
    /// the stack. Used by Controls (`ApplyAndStay`, e.g. toggling
    /// a filter chip without closing the overlay) and Pins
    /// (`ApplyAndStay`, e.g. arming a Delete confirmation).
    CommitAndStay(Box<Msg>),
    /// Operator asked to close (Esc / Ctrl-C / dedicated key).
    /// The runtime pops the overlay; no follow-up dispatch.
    Close,
}

/// Every stack entry implements this trait. The dispatcher matches
/// on the `Modal` variant, builds the widget's per-variant
/// context, calls `handle(ctx, key)`, and acts on the
/// [`OverlayOutcome`]. New overlays don't touch the dispatcher
/// beyond adding a match arm that builds their context and calls
/// the trait method.
///
/// The `Ctx<'a>` associated type carries any live App state the
/// widget needs to see on every event. Widgets that need no
/// context set `type Ctx<'a> = ()`; widgets that need borrowed
/// App data (Controls, Pins, Search) declare a lifetime-carrying
/// context type and the dispatcher builds a fresh borrow per
/// event.
pub trait Overlay {
    /// Live App context the widget consumes on every event. `()`
    /// for context-free widgets (Help, ValueModal, Viewer,
    /// Rename); a lifetime-parameterized reference type for
    /// Controls / Pins / Search.
    type Ctx<'a>;

    /// Handle one key event against fresh `ctx`.
    fn handle(
        &mut self,
        ctx: Self::Ctx<'_>,
        key: ratatui::crossterm::event::KeyEvent,
    ) -> OverlayOutcome;
}

/// Wrapper that adapts the generic [`TextInputState`] widget into
/// an [`Overlay`] impl for the rename modal. The widget-level
/// `InputOutcome::Confirm(String)` maps to
/// `Msg::CommitRename(String)` here so the trait's uniform
/// `Commit(Msg)` outcome carries the specific rename intent.
/// Cancel maps to `Close`; incidental keystrokes stay `Consumed`.
///
/// The wrapper exists because [`TextInputState`] is used by more
/// than the rename overlay (H-EXT config editing surfaces reuse
/// it), so its outcome enum stays generic and the modal-specific
/// mapping lives here.
///
/// [`TextInputState`]: crate::tui::widgets::input::TextInputState
#[derive(Debug, Clone)]
pub struct RenameOverlayState {
    inner: crate::tui::widgets::input::TextInputState,
}

impl RenameOverlayState {
    /// Wrap a fresh text-input state seeded for a rename.
    pub fn new(inner: crate::tui::widgets::input::TextInputState) -> Self {
        Self { inner }
    }

    /// Access the wrapped state so the renderer and existing
    /// helpers can keep working with `TextInputState` directly.
    pub fn inner(&self) -> &crate::tui::widgets::input::TextInputState {
        &self.inner
    }

    /// Mutable access to the wrapped state — used by helpers that
    /// seed the input buffer before opening the overlay.
    pub fn inner_mut(&mut self) -> &mut crate::tui::widgets::input::TextInputState {
        &mut self.inner
    }
}

impl Overlay for RenameOverlayState {
    type Ctx<'a> = ();

    fn handle(&mut self, _ctx: (), key: ratatui::crossterm::event::KeyEvent) -> OverlayOutcome {
        use crate::tui::widgets::input::InputOutcome;
        match self.inner.handle_key(key) {
            InputOutcome::Continue => OverlayOutcome::Consumed,
            InputOutcome::Cancel => OverlayOutcome::Close,
            InputOutcome::Confirm(value) => {
                OverlayOutcome::Commit(Box::new(Msg::CommitRename(value)))
            }
        }
    }
}
