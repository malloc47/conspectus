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
//! The story is phased: `Modal` grows one variant per migration
//! wave (see `docs/backlog.md` §H-TUI-003). Each wave replaces one
//! `Option<...OverlayState>` field on `App` with a `Modal` variant
//! and moves its input/dispatch through the trait's outcome.

use crate::tui::Msg;

/// A modal surface currently open on the stack. The variant carries
/// the overlay's live state; the [`Overlay`] impl handles input and
/// rendering.
///
/// Growing this enum is the migration point for each overlay wave.
/// Ordering here matches the migration order recorded in the
/// backlog so reviewers can trace which surfaces are on the stack
/// vs still on `App`'s `Option<...>` fields.
#[derive(Debug, Clone)]
pub enum Modal {
    Help(crate::tui::widgets::help::HelpOverlayState),
    /// Controls overlay (ADR 0031, F8-004). Doesn't implement
    /// [`Overlay`] yet — the widget's `handle_key` reads a live
    /// [`crate::tui::widgets::controls::ControlsContext`] borrowed
    /// from `App` on every event, and the trait's context-free
    /// signature can't carry it. Dispatch stays through the
    /// specialized `handle_controls_overlay_key` runtime helper
    /// until either the trait grows an associated context type or
    /// the widget internalizes its state.
    Controls(crate::tui::widgets::controls::ControlsOverlayState),
    /// Pins overlay (ADR 0057). Same context-carrying shape as
    /// `Controls` — the widget's `handle_key` reads a
    /// [`crate::tui::widgets::pins::PinsContext`] on every event —
    /// so it doesn't implement [`Overlay`] yet.
    Pins(crate::tui::widgets::pins::PinsOverlayState),
}

/// What an overlay wants the runtime to do after a single key
/// event.
///
/// `Commit` boxes its [`Msg`] because the message enum is large
/// (over 256 bytes for `Msg::SetData` and friends) and the vast
/// majority of `OverlayOutcome` values are `Consumed` / `Close`;
/// paying an indirection per commit is cheaper than fattening
/// every stack-owned outcome.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlayOutcome {
    /// Overlay handled the key — nothing else to do. The stack
    /// stays as-is.
    Consumed,
    /// Overlay committed a value. The runtime pops the overlay,
    /// then dispatches the [`Msg`] through the reducer + executor.
    Commit(Box<Msg>),
    /// Operator asked to close (Esc / Ctrl-C / dedicated key).
    /// The runtime pops the overlay; no follow-up dispatch.
    Close,
}

/// Every stack entry implements this trait. The dispatcher matches
/// on the `Modal` variant, calls `handle`, and acts on the
/// [`OverlayOutcome`]. New overlays don't touch the dispatcher —
/// they add a `Modal` variant and a trait impl.
pub trait Overlay {
    fn handle(&mut self, key: ratatui::crossterm::event::KeyEvent) -> OverlayOutcome;
}
