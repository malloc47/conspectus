//! Overlays: the contract every modal surface keeps, and the stack a host
//! keeps them on.
//!
//! This is a layer 2 convention (house rules 4 and 5). A prompt, a menu,
//! a dialog, a form, a reader, a palette, or a full-screen viewer that
//! floats over a program's screen is an *overlay*. Each one implements
//! one trait, so a host routes keys to whichever is on top and draws all
//! of them the same way, without a branch per overlay:
//!
//! - **The contract.** [`Overlay<M, Cx>`](Overlay) has two calls.
//!   [`handle`](Overlay::handle) takes one key press and the host's
//!   context `&Cx` and returns an [`Outcome<M>`](Outcome): the key was
//!   used, the overlay closes, or it hands the host a message `M`
//!   (closing as it does, or staying open). [`render`](Overlay::render)
//!   draws it over an area with the same context and the theme. The
//!   overlay performs no effect: what a message does is the host's.
//! - **The stack.** [`ModalStack<E>`](ModalStack) holds the open
//!   overlays, bottom to top, as entries of a type the host chooses: its
//!   own enum of overlays, or a boxed trait object. A key goes to the top
//!   entry only ([`route`](ModalStack::route)); the host settles the
//!   outcome ([`settle`](ModalStack::settle)), which closes the top entry
//!   for [`Outcome::Close`] and [`Outcome::Commit`] and gives back the
//!   message to handle. Every entry is drawn, bottom to top
//!   ([`draw`](ModalStack::draw)): each one clears what it covers, so a
//!   smaller overlay shows the one under it around its frame, and an
//!   overlay that [fills the screen](Overlay::fills_screen) hides
//!   everything below it.
//! - **The host owns the stack.** It keeps the stack beside (or lends it
//!   out of) its own state and passes that state as `&Cx` while the stack
//!   holds its entries mutably, so an overlay reads live host data on
//!   every key and every frame instead of a copy taken when it opened.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use ratatui::layout::Rect;
//! use tuikit::frame::{Lines, ModalFrame, ModalSize};
//! use tuikit::keys::{MENU_KEYS, MenuKey, resolve};
//! use tuikit::overlay::{ModalStack, Outcome, Overlay};
//! use tuikit::theme::Theme;
//!
//! /// The host's messages and the context its overlays read.
//! #[derive(Debug, PartialEq)]
//! enum Msg {
//!     Open(&'static str),
//! }
//! struct Files {
//!     names: Vec<&'static str>,
//! }
//!
//! /// A menu over the host's file names.
//! struct Picker {
//!     cursor: usize,
//! }
//!
//! impl Overlay<Msg, Files> for Picker {
//!     fn handle(&mut self, key: KeyEvent, files: &Files) -> Outcome<Msg> {
//!         match resolve(MENU_KEYS, key) {
//!             Some(MenuKey::Abort | MenuKey::Close) => Outcome::Close,
//!             Some(MenuKey::Down) => {
//!                 self.cursor = (self.cursor + 1) % files.names.len();
//!                 Outcome::Consumed
//!             }
//!             Some(MenuKey::Pick) => Outcome::Commit(Msg::Open(files.names[self.cursor])),
//!             Some(MenuKey::Up) | None => Outcome::Consumed,
//!         }
//!     }
//!
//!     fn render(&mut self, area: Rect, buf: &mut Buffer, files: &Files, theme: &Theme) {
//!         let lines = files.names.iter().map(|name| (*name).into()).collect();
//!         ModalFrame::new(theme, ModalSize::MENU)
//!             .title("Open")
//!             .render(area, buf, Lines::new(lines).following(self.cursor..self.cursor + 1));
//!     }
//! }
//!
//! let files = Files { names: vec!["notes", "plan"] };
//! let mut stack: ModalStack<Picker> = ModalStack::new();
//! stack.push(Picker { cursor: 0 });
//!
//! let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
//! let outcome = stack.route(press(KeyCode::Down), &files);
//! assert_eq!(outcome, Some(Outcome::Consumed));
//! let outcome = stack.route(press(KeyCode::Enter), &files).expect("a menu is open");
//! let settled = stack.settle(outcome);
//! assert_eq!(settled.message, Some(Msg::Open("plan")));
//! assert!(settled.closed.is_some() && stack.is_empty(), "a commit closes the menu");
//!
//! // Drawing walks the stack bottom to top; an empty stack draws nothing.
//! let theme = Theme::default();
//! let mut buf = Buffer::empty(Rect::new(0, 0, 80, 24));
//! stack.draw(buf.area, &mut buf, &files, &theme);
//! ```
//!
//! # What a host keeps
//!
//! Its entry type and the one place that lists its overlays; what each
//! message does; any bookkeeping when an overlay closes (the entry
//! [`settle`](ModalStack::settle) closed is handed back for it); its base
//! screen, which it draws before the stack unless an entry fills the
//! screen; and its key tables, through which each overlay resolves its
//! keys (see [`keys`](crate::keys)), with the close keys of house rule 4.

mod contract;
mod outcome;
mod stack;

pub use contract::Overlay;
pub use outcome::Outcome;
pub use stack::{ModalStack, Settled};

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
