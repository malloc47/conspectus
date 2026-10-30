//! Action menus: a short list of actions in a small modal, one of which
//! the user picks.
//!
//! This is a layer 1 control (house rules 1 to 5, 8, 13, and 14). A
//! program offers the actions that apply to something (a row, a
//! selection, the program itself) in a menu, and every such menu should
//! move, confirm, scroll, and close the same way, or one stops at its
//! ends and the next wraps, one asks twice before a destructive action
//! and the next does not say it is waiting, and on a short screen one
//! keeps its keys in view while another loses them:
//!
//! - **Entries as data.** An [`ActionMenu<M>`](ActionMenu) holds a title
//!   and a list of [`Entry`]s: a label, an optional detail line, an
//!   optional shortcut, whether it can run now, how it [`Confirm`]s, and
//!   the host's message `M` it commits.
//! - **One cursor.** Rows are drawn through
//!   [`selection::row`](crate::selection::row): the pointer in the
//!   cursor gutter and the theme's selection pair. The cursor wraps from
//!   the last entry to the first (house rule 2) and steps over disabled
//!   entries.
//! - **Keys from the standard table.** [`MENU_KEYS`] moves (`↑`/`↓`,
//!   `k`/`j`, `Ctrl-P`/`Ctrl-N`), picks (`Enter`), and closes (`Esc`,
//!   `q`, `Ctrl-C`), and generates the footer. A shortcut picks its entry
//!   from anywhere; a key the table binds never reaches one.
//! - **Inline confirmation.** An entry that asks for a second `Enter`
//!   arms on the first, and its row then says what the next `Enter` will
//!   do. Moving the cursor disarms it; closing the menu says nothing.
//! - **Further questions.** An entry that leads to a question of its own
//!   (a dialog with a body, a prompt for a value) [opens a
//!   step](Entry::opens_step): picking it reports its message and the
//!   menu stays open under the step the host opens
//!   ([`ModalStack::push_step`](crate::overlay::ModalStack::push_step)),
//!   so `Esc` there comes back to the menu.
//! - **Scrolling with a fixed footer.** The menu is the menu-sized modal
//!   frame, as tall as its rows. On a screen too short for them the
//!   footer keeps the bottom rows and the entries scroll in a window the
//!   menu keeps, moving no further than the cursor needs, with the shared
//!   scrollbar beside them.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use ratatui::layout::Rect;
//! use tuikit::action_menu::{ActionMenu, Confirm, Entry};
//! use tuikit::keys::ch;
//! use tuikit::overlay::{Outcome, Overlay};
//! use tuikit::testkit::buffer::rows;
//! use tuikit::theme::Theme;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Msg {
//!     Open,
//!     Remove,
//! }
//!
//! let mut menu = ActionMenu::new(
//!     "notes.txt",
//!     vec![
//!         Entry::new("Open", Msg::Open).detail("show the file").shortcut(ch('o')),
//!         Entry::new("Remove", Msg::Remove).confirm(Confirm::Inline),
//!     ],
//! );
//! let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
//!
//! // Up from the first entry wraps to the last, which asks twice.
//! menu.handle(press(KeyCode::Up), &());
//! assert_eq!(menu.handle(press(KeyCode::Enter), &()), Outcome::Consumed);
//! assert_eq!(menu.armed(), Some(1));
//! assert_eq!(menu.handle(press(KeyCode::Enter), &()), Outcome::Commit(Msg::Remove));
//!
//! // A shortcut picks its entry from anywhere.
//! assert_eq!(menu.handle(press(KeyCode::Char('o')), &()), Outcome::Commit(Msg::Open));
//!
//! let theme = Theme::default();
//! let mut buf = Buffer::empty(Rect::new(0, 0, 60, 12));
//! menu.draw(buf.area, &mut buf, &theme);
//! let drawn = rows(&buf).join("\n");
//! assert!(drawn.contains("► o  Open"), "{drawn}");
//! assert!(drawn.contains("↑/↓ move · Enter pick · Esc close"), "{drawn}");
//! ```
//!
//! # What a host keeps
//!
//! Which entries a menu offers and in what order, their labels and
//! details, which of them confirm, and what each commits: a host builds
//! the entries from its own state each time it opens the menu, and the
//! menu reports the chosen entry's message. For an entry that opens a
//! step, the host builds the step from the message (a dialog, a prompt)
//! and opens it with
//! [`ModalStack::push_step`](crate::overlay::ModalStack::push_step): a
//! commit or a dismissal there closes the menu too, and `Esc` there gives
//! the menu back its keys.

mod body;
mod entry;

pub use body::confirm_note;
pub use entry::{Confirm, Entry};

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use crate::frame::{ModalFrame, ModalSize};
use crate::keys::{Footer, MENU_KEYS, MenuKey, key_label, resolve};
use crate::overlay::{Outcome, Overlay};
use crate::scroll::ScrollWindow;
use crate::selection::Ends;
use crate::theme::Theme;

use body::{Body, View};

/// How large every menu is: the menu-sized modal frame, as tall as its
/// rows.
pub const MENU_SIZE: ModalSize = ModalSize::MENU;

/// A modal list of actions: a title, its entries, and the footer the
/// standard menu table generates.
///
/// The menu implements [`Overlay<M, Cx>`] for every `Cx` and reads
/// nothing from the context. Drawing keeps the scroll window it showed,
/// so the next key moves the window no further than the cursor needs.
///
/// A host builds the entries from its own state when the menu opens and
/// puts the menu on its stack:
///
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use ratatui::layout::Rect;
/// use tuikit::action_menu::{ActionMenu, Confirm, Entry};
/// use tuikit::keys::ch;
/// use tuikit::overlay::ModalStack;
/// use tuikit::theme::Theme;
///
/// #[derive(Clone, Debug, PartialEq)]
/// enum Msg {
///     Open(String),
///     Share(String),
///     Delete(String),
/// }
///
/// struct File {
///     name: String,
///     shared: bool,
/// }
///
/// /// The actions on `file`: sharing it again cannot run, and deleting
/// /// it asks twice.
/// fn file_menu(file: &File) -> ActionMenu<Msg> {
///     let mut share = Entry::new("Share", Msg::Share(file.name.clone()));
///     share = if file.shared {
///         share.detail("already shared").disabled()
///     } else {
///         share.detail("send a link").shortcut(ch('s'))
///     };
///     ActionMenu::new(
///         file.name.as_str(),
///         vec![
///             Entry::new("Open", Msg::Open(file.name.clone())).detail("show it"),
///             share,
///             Entry::new("Delete", Msg::Delete(file.name.clone()))
///                 .detail("remove it for good")
///                 .confirm(Confirm::Inline),
///         ],
///     )
/// }
///
/// let file = File { name: "plan.txt".into(), shared: true };
/// let mut stack: ModalStack<ActionMenu<Msg>> = ModalStack::new();
/// stack.push(file_menu(&file));
/// let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
///
/// // Down steps over the entry that cannot run; Delete asks twice.
/// stack.route(press(KeyCode::Down), &file);
/// assert_eq!(stack.top().map(ActionMenu::cursor), Some(2));
/// let first = stack.route(press(KeyCode::Enter), &file).expect("a menu is open");
/// assert!(stack.settle(first).message.is_none(), "armed, still open");
/// let second = stack.route(press(KeyCode::Enter), &file).expect("a menu is open");
/// let settled = stack.settle(second);
/// assert_eq!(settled.message, Some(Msg::Delete("plan.txt".into())));
/// assert!(stack.is_empty(), "the pick closed the menu");
///
/// // Drawn in the menu-sized frame with its generated footer.
/// let mut menu = file_menu(&File { name: "plan.txt".into(), shared: false });
/// let mut buf = Buffer::empty(Rect::new(0, 0, 70, 14));
/// menu.draw(buf.area, &mut buf, &Theme::default());
/// assert!(menu.shadowed_shortcuts().is_empty());
/// ```
#[derive(Clone, Debug)]
pub struct ActionMenu<M> {
    title: String,
    entries: Vec<Entry<M>>,
    cursor: usize,
    armed: Option<usize>,
    window: ScrollWindow,
}

impl<M> ActionMenu<M> {
    /// A menu titled `title` over `entries`, with the cursor on the first
    /// entry that can run. A menu with no entries, or none that can run,
    /// draws its footer and closes on its close keys; `Enter` does
    /// nothing in it.
    pub fn new(title: impl Into<String>, entries: Vec<Entry<M>>) -> Self {
        let cursor = entries.iter().position(|entry| entry.enabled).unwrap_or(0);
        Self {
            title: title.into(),
            entries,
            cursor,
            armed: None,
            window: ScrollWindow::default(),
        }
    }

    /// The title on the menu's top border.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The entries, in their order.
    pub fn entries(&self) -> &[Entry<M>] {
        &self.entries
    }

    /// The index of the entry the cursor is on. It is always an entry
    /// that can run, unless none can.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The entry the cursor is on, when it can run.
    pub fn selected(&self) -> Option<&Entry<M>> {
        self.entries.get(self.cursor).filter(|entry| entry.enabled)
    }

    /// The entry waiting for its confirming `Enter`, if one is armed.
    pub fn armed(&self) -> Option<usize> {
        self.armed
    }

    /// The scroll window of the last draw: which rows of the entries were
    /// on screen.
    pub fn window(&self) -> ScrollWindow {
        self.window
    }

    /// The shortcuts no key press can reach, as their key labels: each
    /// one a key the menu's table binds (`j`, `q`, `Enter`), or a key an
    /// earlier entry already uses. Empty when every shortcut works; a
    /// host checks the menus it builds with it.
    pub fn shadowed_shortcuts(&self) -> Vec<String> {
        let mut shadowed = Vec::new();
        for (index, entry) in self.entries.iter().enumerate() {
            let Some(key) = entry.shortcut else {
                continue;
            };
            let press = key.example();
            let taken = resolve(MENU_KEYS, press).is_some()
                || self.entries[..index]
                    .iter()
                    .filter_map(|earlier| earlier.shortcut)
                    .any(|earlier| earlier.matches_event(press));
            if taken {
                shadowed.push(key_label(&key));
            }
        }
        shadowed
    }

    /// Draws the menu over `area`: the frame, the entries, and the
    /// footer, and keeps the window it showed. A host whose overlay holds
    /// the menu as one of its levels draws it with this; on a stack,
    /// [`Overlay::render`] does.
    pub fn draw(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let frame = ModalFrame::new(theme, MENU_SIZE).title(self.title.as_str());
        let body = Body {
            view: View {
                entries: &self.entries,
                cursor: self.cursor,
                armed: self.armed,
                theme,
            },
            window: self.window,
            width: 0,
            layout: None,
            footer: Vec::new(),
        };
        let window = frame.render(area, buf, body);
        self.window = window;
    }

    /// Moves the cursor one entry by `delta`, wrapping and stepping over
    /// entries that cannot run, and disarms.
    fn step(&mut self, delta: i32) {
        self.armed = None;
        let len = self.entries.len();
        if !self.entries.iter().any(|entry| entry.enabled) {
            return;
        }
        let mut next = self.cursor;
        for _ in 0..len {
            next = Ends::Wrap.step(next, len, delta);
            if self.entries[next].enabled {
                break;
            }
        }
        self.cursor = next;
    }

    /// The entry whose shortcut `key` is, if any.
    fn shortcut_for(&self, key: KeyEvent) -> Option<usize> {
        self.entries.iter().position(|entry| {
            entry
                .shortcut
                .is_some_and(|shortcut| shortcut.matches_event(key))
        })
    }
}

impl<M: Clone> ActionMenu<M> {
    /// Picks entry `index`: arms it when it asks for a second `Enter` and
    /// is not armed yet, reports it and stays open when it opens a step,
    /// commits it otherwise, and does nothing when it cannot run.
    fn pick(&mut self, index: usize) -> Outcome<M> {
        let Some(entry) = self.entries.get(index).filter(|entry| entry.enabled) else {
            return Outcome::Consumed;
        };
        if entry.confirm == Confirm::Inline && self.armed != Some(index) {
            self.armed = Some(index);
            return Outcome::Consumed;
        }
        self.armed = None;
        if entry.opens_step {
            Outcome::Stay(entry.outcome.clone())
        } else {
            Outcome::Commit(entry.outcome.clone())
        }
    }
}

impl<M: Clone, Cx: ?Sized> Overlay<M, Cx> for ActionMenu<M> {
    /// The arrows and their aliases move; `Enter` picks the cursor's
    /// entry; `Esc` closes the menu (`Close`) and `q` and `Ctrl-C` dismiss
    /// it (`Dismiss`), saying nothing; an entry's shortcut moves the
    /// cursor to it and picks it. Every other key is swallowed.
    fn handle(&mut self, key: KeyEvent, _cx: &Cx) -> Outcome<M> {
        match resolve(MENU_KEYS, key) {
            Some(MenuKey::Abort) => Outcome::Dismiss,
            Some(MenuKey::Close) => Outcome::Close,
            Some(MenuKey::Up) => {
                self.step(-1);
                Outcome::Consumed
            }
            Some(MenuKey::Down) => {
                self.step(1);
                Outcome::Consumed
            }
            Some(MenuKey::Pick) => self.pick(self.cursor),
            None => match self.shortcut_for(key) {
                Some(index) if self.entries[index].enabled => {
                    if index != self.cursor {
                        self.cursor = index;
                        self.armed = None;
                    }
                    self.pick(index)
                }
                _ => Outcome::Consumed,
            },
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _cx: &Cx, theme: &Theme) {
        self.draw(area, buf, theme);
    }

    /// The standard menu table's footer.
    fn hints(&self) -> Footer {
        Footer::new(MENU_KEYS)
    }
}

#[cfg(test)]
#[path = "action_menu_tests.rs"]
mod tests;
