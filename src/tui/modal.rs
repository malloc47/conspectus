//! The modal stack (ADR 0085 contract 3, ADR 0161).
//!
//! Every open overlay is an entry of [`Modal`] on the App's
//! [`tuikit::overlay::ModalStack`]. Each entry implements
//! [`Overlay<Msg, App>`](tuikit::overlay::Overlay): `handle` takes a key with
//! the App as its context and returns an [`Outcome`], and `render` draws the
//! entry with the same context. So:
//!
//! - **Keys** go to the top entry only. The loops, the fixture TUI, and the
//!   snapshot driver resolve every key an open modal owns to one
//!   `Action::ModalKey`, which `runtime::modal_key` routes: the stack closes
//!   the top entry for `Close` and `Commit`, and what an entry commits goes
//!   to the caller's executor.
//! - **Drawing** walks the stack bottom to top after the panes, so a modal
//!   opened over another shows the lower one around its frame. A full-screen
//!   viewer fills the screen, and nothing under it is drawn, the panes
//!   included.
//! - **The context** is the App itself. The App lends its stack out for the
//!   call ([`App::with_modals`]), so an entry is borrowed mutably while the
//!   App is borrowed shared: Controls and the search overlay read the live
//!   App instead of a copy taken before each key. While the stack is lent
//!   out the App's own stack is empty, so an entry reads its own state from
//!   itself, never through the App's modal accessors.
//!
//! Adding a modal is a state struct, an `Overlay<Msg, App>` impl (in
//! `modal/entries.rs` beside the others), and one line in the registration
//! below. The keymap, the loops, and the draw take no new branch, and the
//! registry test (`consistency_tests/registry.rs`) fails until the new
//! modal is built there, where it is checked for routing, drawing, and the
//! close keys.
//!
//! [`App::with_modals`]: crate::tui::App::with_modals

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;
use tuikit::overlay::{Outcome, Overlay};

use crate::tui::{App, Msg};

mod entries;

/// The App's modal stack.
pub type ModalStack = tuikit::overlay::ModalStack<Modal>;

/// Declares [`Modal`] from the one list of entries: the enum, every
/// entry's name, and the [`Overlay`] impl that hands each call to the
/// entry. A modal registered here is routed and drawn with no other
/// change.
macro_rules! modals {
    ($($(#[$doc:meta])* $variant:ident($entry:ty),)*) => {
        /// A modal surface open on the stack. Each variant holds the
        /// overlay's live state, which implements
        /// [`Overlay<Msg, App>`](Overlay).
        #[derive(Debug, Clone)]
        pub enum Modal {
            $($(#[$doc])* $variant($entry),)*
        }

        impl Modal {
            /// Every registered modal, by name, in registration order.
            pub const NAMES: &'static [&'static str] = &[$(stringify!($variant)),*];

            /// This modal's name, as [`Modal::NAMES`] lists it.
            pub fn name(&self) -> &'static str {
                match self {
                    $(Self::$variant(_) => stringify!($variant),)*
                }
            }
        }

        impl Overlay<Msg, App> for Modal {
            fn handle(&mut self, key: KeyEvent, app: &App) -> Outcome<Msg> {
                match self {
                    $(Self::$variant(entry) => entry.handle(key, app),)*
                }
            }

            fn render(
                &mut self,
                area: Rect,
                buf: &mut Buffer,
                app: &App,
                theme: &tuikit::theme::Theme,
            ) {
                match self {
                    $(Self::$variant(entry) => entry.render(area, buf, app, theme),)*
                }
            }

            fn fills_screen(&self) -> bool {
                match self {
                    $(Self::$variant(entry) => Overlay::<Msg, App>::fills_screen(entry),)*
                }
            }

            fn takes_text(&self) -> bool {
                match self {
                    $(Self::$variant(entry) => Overlay::<Msg, App>::takes_text(entry),)*
                }
            }
        }
    };
}

modals! {
    /// `?` help page.
    Help(crate::tui::widgets::help::HelpOverlayState),
    /// Controls overlay (ADR 0031): view, grouping, filters, sort, and
    /// search settings, applied as they change. Reads the App's settings
    /// on every key and every frame.
    Controls(crate::tui::widgets::controls::ControlsOverlayState),
    /// Rename prompt (ADR 0029): the framework's text prompt, whose
    /// confirm the opener's closure turns into the rename it means (a
    /// session's alias, a live mux's name, or a pin's name).
    Rename(tuikit::text_prompt::TextPrompt<Msg>),
    /// `/` quick select and `s` content search. Ranks the App's rows and
    /// content results on every key.
    Search(crate::tui::widgets::search::SearchOverlayState),
    /// `o` full-value modal.
    ValueModal(crate::tui::widgets::value_modal::ValueModalState),
    /// `!` Messages overlay (ADR 0105), over the App's message log.
    Messages(crate::tui::widgets::messages::MessagesOverlayState),
    /// Full-screen transcript viewer (ADR 0052). A nested reducer: its
    /// keys become [`Msg::Viewer`] and [`Msg::ViewerFind`], whose reducer
    /// arms apply them to this entry, and close it when the viewer's own
    /// reducer says so.
    Viewer(crate::viewer::state::ViewerState),
    /// Full-screen viewer over a mux pane's scrollback, opened from a pane
    /// search result; `a` attaches. Boxed to keep [`Modal`] small.
    PaneViewer(Box<crate::tui::widgets::pane_viewer::PaneViewerState>),
    /// `w` worktree action menu: the framework's action menu over the
    /// worktree verbs for the selection, whose question entries open
    /// their branch prompt, confirmation, or choice as a step over it.
    /// Boxed to keep [`Modal`] small.
    WorktreeMenu(Box<tuikit::action_menu::ActionMenu<Msg>>),
    /// A dialog: a confirmation, a choice, or a notice, whose answers
    /// are messages. Opened as a step of the overlay that asked (a
    /// worktree confirmation over its menu) or on its own (`X`'s
    /// close-down choice, the empty Clean up notice). Boxed to keep
    /// [`Modal`] small.
    Dialog(Box<tuikit::dialog::Dialog<Msg>>),
    /// A text prompt that is a step of another overlay (the worktree
    /// menu's branch prompt), whose confirm its closure turns into the
    /// message it means.
    Prompt(tuikit::text_prompt::TextPrompt<Msg>),
    /// `n` bare tmux new-session form (ADR 0095): the framework's form,
    /// whose submit its closure turns into the new mux.
    NewMux(tuikit::form::Form<Msg>),
    /// `m` mux action menu (ADR 0096): the framework's action menu over
    /// the two ways to start a mux. Boxed to keep [`Modal`] small.
    MuxMenu(Box<tuikit::action_menu::ActionMenu<Msg>>),
    /// Mux launch form: a harness in a fresh mux (ADR 0096), the
    /// framework's form with the fields that follow others kept in step.
    /// Boxed to keep [`Modal`] small.
    MuxLaunch(Box<crate::tui::widgets::mux_launch::MuxLaunchForm>),
    /// The pin edit form: every field a pin can change, with the layer
    /// each value comes from.
    PinEdit(Box<crate::tui::widgets::pin_edit::PinEditForm>),
    /// `.` row action menu, and the menu that unpins one of several
    /// pins: the framework's action menu over the verbs for the selected
    /// row, each committing a [`Msg`].
    RowMenu(Box<tuikit::action_menu::ActionMenu<Msg>>),
    /// The History view's Clean up checklist (ADR 0140), when there is
    /// something to clean up: the framework's checklist over the records
    /// past their reap age, whose confirm forgets the checked ones; a
    /// [`Modal::Dialog`] notice says when there is nothing. Boxed to keep
    /// [`Modal`] small.
    CleanUp(Box<tuikit::checklist::Checklist<Msg>>),
    /// A pick-one list with type-to-filter: where a pin points next, or a
    /// remembered search. The framework's palette over the candidates,
    /// whose pick its closure turns into the message it means. Boxed to
    /// keep [`Modal`] small.
    Picker(Box<crate::tui::widgets::picker::Picker>),
}
