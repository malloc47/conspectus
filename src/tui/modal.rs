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
//! layout suite's coverage check (`consistency_tests/layout.rs`) fails
//! until the new modal is built there.
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
    /// Rename prompt (ADR 0029, ADR 0030): the text-input widget with its
    /// confirm mapped to [`Msg::CommitRename`].
    Rename(RenameOverlayState),
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
    /// `w` worktree action menu: a list, a branch prompt, confirmations,
    /// and the close-down choice, with the worktree facts captured when
    /// it opened.
    WorktreeMenu(Box<crate::tui::widgets::worktree_menu::WorktreeMenuState>),
    /// `n` bare tmux new-session form (ADR 0095).
    NewMux(crate::tui::widgets::new_mux::NewMuxFormState),
    /// `m` mux action menu (ADR 0096).
    MuxMenu(crate::tui::widgets::mux_menu::MuxMenuState),
    /// Mux launch form: a harness in a fresh mux (ADR 0096). Boxed to keep
    /// [`Modal`] small.
    MuxLaunch(Box<crate::tui::widgets::mux_launch::MuxLaunchFormState>),
    /// The pin edit form: every field a pin can change, with the layer
    /// each value comes from.
    PinEdit(Box<crate::tui::widgets::pin_edit::PinEditFormState>),
    /// `.` row action menu: the verbs for the selected row, each
    /// committing a [`Msg`].
    RowMenu(Box<crate::tui::widgets::row_menu::RowMenuState>),
    /// The History view's Clean up checklist (ADR 0140).
    CleanUp(Box<crate::tui::widgets::clean_up::CleanUpState>),
    /// A pick-one list with type-to-filter: where a pin points next, or a
    /// remembered search.
    Picker(Box<crate::tui::widgets::picker::PickerState>),
}

/// The rename prompt: the generic text-input widget, whose confirm maps
/// to [`Msg::CommitRename`]. The text-input state is shared with other
/// prompts, so its own outcome stays generic and the rename mapping lives
/// here.
#[derive(Debug, Clone)]
pub struct RenameOverlayState {
    inner: crate::tui::widgets::input::TextInputState,
}

impl RenameOverlayState {
    /// Wrap a fresh text-input state seeded for a rename.
    pub fn new(inner: crate::tui::widgets::input::TextInputState) -> Self {
        Self { inner }
    }

    /// The wrapped text-input state.
    pub fn inner(&self) -> &crate::tui::widgets::input::TextInputState {
        &self.inner
    }

    /// The wrapped text-input state, mutably, for seeding the buffer.
    pub fn inner_mut(&mut self) -> &mut crate::tui::widgets::input::TextInputState {
        &mut self.inner
    }

    /// One key into the prompt: a confirm commits the rename; a cancel
    /// or a dismissal closes it.
    pub fn handle_key(&mut self, key: KeyEvent) -> Outcome<Msg> {
        use crate::tui::widgets::input::InputOutcome;
        match self.inner.handle_key(key) {
            InputOutcome::Continue => Outcome::Consumed,
            InputOutcome::Cancel | InputOutcome::Abort => Outcome::Close,
            InputOutcome::Confirm(value) => Outcome::Commit(Msg::CommitRename(value)),
        }
    }
}
