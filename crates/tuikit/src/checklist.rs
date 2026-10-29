//! Checklists: a short list of items in a small modal, any number of
//! which the user checks before confirming.
//!
//! This is a layer 1 control (house rules 1, 2, 4, 5, 8, 12, 13, and 14).
//! A program asks which of several things to act on (the values a filter
//! keeps, the records a clean-up removes) with a checklist, and every
//! such list should move, check, scroll, and close the same way, or one
//! wraps at its ends and the next stops, one can check everything at once
//! and the next cannot, and on a short screen one keeps its keys in view
//! while another loses them:
//!
//! - **Items as data.** A [`Checklist<M>`](Checklist) holds a title and a
//!   list of [`Item`]s: a label, an optional detail, whether it is
//!   checked, and whether it can change. An item that cannot change is
//!   drawn receding and keeps its check.
//! - **One cursor.** Rows are drawn through
//!   [`selection::row`](crate::selection::row): the pointer in the
//!   cursor gutter and the theme's selection pair. The cursor wraps from
//!   the last item to the first (house rule 2) and steps over items that
//!   cannot change; `PgUp`/`PgDn` move it by the rows shown and the ends
//!   keys take it to the first or last item, stopping there.
//! - **Keys from the standard table.** [`CHECKLIST_KEYS`] moves, checks
//!   the cursor's item (`Space`), checks or unchecks everything that can
//!   change (`a`, `n`), confirms (`Enter`), and closes (`Esc`, `q`,
//!   `Ctrl-C`), and generates the footer.
//! - **A count.** The bottom border says how many items are checked
//!   (`2 of 5 selected`), so the total is in view whatever is scrolled.
//! - **The host decides what a confirm means.** `Enter` hands the checked
//!   indices, in order, to the host's `on_confirm`, which answers with the
//!   [`Outcome`] the checklist reports: a commit of the host's message, a
//!   close when there is nothing to do, or anything else an overlay can
//!   answer.
//! - **Scrolling with a fixed footer.** The checklist is the compact
//!   modal frame, as tall as its rows. On a screen too short for them
//!   the footer keeps the bottom rows and the items scroll in a window
//!   the checklist keeps, moving no further than the cursor needs, with
//!   the shared scrollbar beside them.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
//! use ratatui::layout::Rect;
//! use tuikit::checklist::{Checklist, Item};
//! use tuikit::overlay::{Outcome, Overlay};
//! use tuikit::testkit::buffer::rows;
//! use tuikit::theme::Theme;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! enum Msg {
//!     Add(Vec<usize>),
//! }
//!
//! let mut list = Checklist::new(
//!     "Toppings",
//!     vec![
//!         Item::new("cheese").checked(true),
//!         Item::new("onion"),
//!         Item::new("pepper"),
//!     ],
//!     |indices: Vec<usize>| Outcome::Commit(Msg::Add(indices)),
//! );
//! let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
//!
//! // Down, then Space checks the second item.
//! list.handle(press(KeyCode::Down), &());
//! list.handle(press(KeyCode::Char(' ')), &());
//! assert_eq!(list.selected(), [0, 1]);
//! assert_eq!(list.summary(), "2 of 3 selected");
//!
//! // `a` checks everything; Enter hands the indices to the host.
//! list.handle(press(KeyCode::Char('a')), &());
//! let done = list.handle(press(KeyCode::Enter), &());
//! assert_eq!(done, Outcome::Commit(Msg::Add(vec![0, 1, 2])));
//!
//! let theme = Theme::default();
//! let mut buf = Buffer::empty(Rect::new(0, 0, 60, 14));
//! list.draw(buf.area, &mut buf, &theme);
//! let drawn = rows(&buf).join("\n");
//! assert!(drawn.contains("► √ onion"), "{drawn}");
//! assert!(drawn.contains("└ 3 of 3 selected "), "{drawn}");
//! ```
//!
//! # What a host keeps
//!
//! Which items a checklist offers and in what order, their labels and
//! details, which start checked and which cannot change, and what a
//! confirm means: a host builds the items from its own state when it
//! opens the checklist, and its `on_confirm` turns the checked indices
//! into its own message (looking up the values it listed). A host that
//! holds the checklist as a level of its own overlay (a settings panel's
//! editor) routes keys to it, takes `Close` as stepping back to its own
//! level, and takes `Dismiss` as dismissing the whole overlay.

mod body;
mod item;

pub use item::Item;

use std::fmt;
use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyEvent;
use ratatui::layout::Rect;

use crate::frame::{ModalFrame, ModalSize};
use crate::keys::{CHECKLIST_KEYS, ChecklistKey, resolve};
use crate::overlay::{Outcome, Overlay};
use crate::scroll::ScrollWindow;
use crate::selection::Ends;
use crate::theme::Theme;

use body::{Body, View};

/// How large every checklist is: the compact modal frame, as tall as its
/// rows.
pub const CHECKLIST_SIZE: ModalSize = ModalSize::COMPACT;

/// The host's decision about the checked indices, shared by every copy
/// of the checklist.
type OnConfirm<M> = Arc<dyn Fn(Vec<usize>) -> Outcome<M> + Send + Sync>;

/// A modal list of items to check: a title, its items, the footer the
/// standard checklist table generates, and the count on its bottom
/// border.
///
/// The checklist implements [`Overlay<M, Cx>`] for every `Cx` and reads
/// nothing from the context. Drawing keeps the scroll window it showed,
/// so the next key moves the window no further than the cursor needs.
/// `on_confirm` is shared behind an [`Arc`], so a checklist clones
/// cheaply.
///
/// A host builds the items from its own state and decides in
/// `on_confirm` what the checked ones mean:
///
/// ```
/// use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use tuikit::checklist::{Checklist, Item};
/// use tuikit::overlay::{ModalStack, Outcome};
///
/// #[derive(Clone, Debug, PartialEq)]
/// enum Msg {
///     Delete(Vec<String>),
/// }
///
/// /// The files to delete, all checked; a file that is open cannot be
/// /// deleted, and deleting nothing closes the list.
/// fn delete_list(files: &[(&str, bool)]) -> Checklist<Msg> {
///     let items = files
///         .iter()
///         .map(|(name, open)| {
///             let item = Item::new(*name);
///             if *open {
///                 item.detail("open in a window").disabled()
///             } else {
///                 item.checked(true)
///             }
///         })
///         .collect();
///     let names: Vec<String> = files.iter().map(|(name, _)| name.to_string()).collect();
///     Checklist::new("Delete", items, move |indices: Vec<usize>| {
///         let chosen: Vec<String> = indices.into_iter().map(|i| names[i].clone()).collect();
///         if chosen.is_empty() {
///             Outcome::Close
///         } else {
///             Outcome::Commit(Msg::Delete(chosen))
///         }
///     })
/// }
///
/// let files = [("a.txt", false), ("b.txt", true), ("c.txt", false)];
/// let mut stack: ModalStack<Checklist<Msg>> = ModalStack::new();
/// stack.push(delete_list(&files));
/// let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
///
/// // Down steps over the open file; Space unchecks c.txt.
/// stack.route(press(KeyCode::Down), &());
/// assert_eq!(stack.top().map(Checklist::cursor), Some(2));
/// stack.route(press(KeyCode::Char(' ')), &());
/// let done = stack.route(press(KeyCode::Enter), &()).expect("a list is open");
/// let settled = stack.settle(done);
/// assert_eq!(settled.message, Some(Msg::Delete(vec!["a.txt".into()])));
/// assert!(stack.is_empty(), "the commit closed the list");
/// ```
#[derive(Clone)]
pub struct Checklist<M> {
    title: String,
    items: Vec<Item>,
    cursor: usize,
    window: ScrollWindow,
    on_confirm: OnConfirm<M>,
}

impl<M> Checklist<M> {
    /// A checklist titled `title` over `items`, with the cursor on the
    /// first item that can change. `on_confirm` gets the indices of the
    /// checked items, in order, when `Enter` is pressed, and its answer
    /// is what the checklist reports. A checklist with no items, or none
    /// that can change, draws its footer and closes on its close keys.
    pub fn new(
        title: impl Into<String>,
        items: Vec<Item>,
        on_confirm: impl Fn(Vec<usize>) -> Outcome<M> + Send + Sync + 'static,
    ) -> Self {
        let cursor = items.iter().position(|item| item.enabled).unwrap_or(0);
        Self {
            title: title.into(),
            items,
            cursor,
            window: ScrollWindow::default(),
            on_confirm: Arc::new(on_confirm),
        }
    }

    /// The title on the checklist's top border.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The items, in their order, each with its check as it stands.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The index of the item the cursor is on. It is always an item that
    /// can change, unless none can.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The indices of the checked items, in order: what `Enter` hands to
    /// `on_confirm`. Items that cannot change count when they are
    /// checked.
    pub fn selected(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| item.checked.then_some(index))
            .collect()
    }

    /// Whether item `index` is checked; `false` past the last item.
    pub fn is_checked(&self, index: usize) -> bool {
        self.items.get(index).is_some_and(|item| item.checked)
    }

    /// The count on the bottom border: how many of the items are checked
    /// (`2 of 5 selected`).
    pub fn summary(&self) -> String {
        let checked = self.items.iter().filter(|item| item.checked).count();
        format!("{checked} of {} selected", self.items.len())
    }

    /// The scroll window of the last draw: which rows of the list were on
    /// screen.
    pub fn window(&self) -> ScrollWindow {
        self.window
    }

    /// Draws the checklist over `area`: the frame with its count, the
    /// items, and the footer, and keeps the window it showed. A host
    /// whose overlay holds the checklist as one of its levels draws it
    /// with this; on a stack, [`Overlay::render`] does.
    pub fn draw(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let frame = ModalFrame::new(theme, CHECKLIST_SIZE)
            .title(self.title.as_str())
            .footer(self.summary());
        let body = Body {
            view: View {
                items: &self.items,
                cursor: self.cursor,
                theme,
            },
            window: self.window,
            rows: None,
            footer: Vec::new(),
        };
        self.window = frame.render(area, buf, body);
    }

    /// Moves the cursor one item by `delta`, wrapping and stepping over
    /// items that cannot change.
    fn step(&mut self, delta: i32) {
        let len = self.items.len();
        if !self.items.iter().any(|item| item.enabled) {
            return;
        }
        let mut next = self.cursor;
        for _ in 0..len {
            next = Ends::Wrap.step(next, len, delta);
            if self.items[next].enabled {
                break;
            }
        }
        self.cursor = next;
    }

    /// Moves the cursor to the item that can change nearest `target`,
    /// looking past it in the direction of travel first (`forward` is
    /// down the list), then back. Stays put when nothing can change.
    fn land(&mut self, target: usize, forward: bool) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        let enabled = |index: &usize| self.items[*index].enabled;
        let target = target.min(len - 1);
        let ahead = if forward {
            (target..len).find(enabled)
        } else {
            (0..=target).rev().find(enabled)
        };
        let behind = || {
            if forward {
                (0..target).rev().find(enabled)
            } else {
                (target + 1..len).find(enabled)
            }
        };
        if let Some(index) = ahead.or_else(behind) {
            self.cursor = index;
        }
    }

    /// Checks or unchecks every item that can change.
    fn check_all(&mut self, checked: bool) {
        for item in self.items.iter_mut().filter(|item| item.enabled) {
            item.checked = checked;
        }
    }
}

impl<M> fmt::Debug for Checklist<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Checklist")
            .field("title", &self.title)
            .field("items", &self.items)
            .field("cursor", &self.cursor)
            .field("window", &self.window)
            .finish_non_exhaustive()
    }
}

impl<M, Cx: ?Sized> Overlay<M, Cx> for Checklist<M> {
    /// The arrows and their aliases move, wrapping; `PgUp`/`PgDn` and the
    /// ends keys move and stop; `Space` checks or unchecks the cursor's
    /// item, and `a` and `n` every item that can change; `Enter` asks
    /// `on_confirm`; `Esc` closes the checklist (`Close`) and `q` and
    /// `Ctrl-C` dismiss it (`Dismiss`), saying nothing. Every other key
    /// is swallowed.
    fn handle(&mut self, key: KeyEvent, _cx: &Cx) -> Outcome<M> {
        let Some(action) = resolve(CHECKLIST_KEYS, key) else {
            return Outcome::Consumed;
        };
        let page = self.window.page();
        let last = self.items.len().saturating_sub(1);
        match action {
            ChecklistKey::Abort => return Outcome::Dismiss,
            ChecklistKey::Cancel => return Outcome::Close,
            ChecklistKey::Confirm => return (self.on_confirm)(self.selected()),
            ChecklistKey::Up => self.step(-1),
            ChecklistKey::Down => self.step(1),
            ChecklistKey::PageUp => self.land(self.cursor.saturating_sub(page), false),
            ChecklistKey::PageDown => self.land(self.cursor.saturating_add(page), true),
            ChecklistKey::Top => self.land(0, true),
            ChecklistKey::Bottom => self.land(last, false),
            ChecklistKey::Toggle => {
                if let Some(item) = self.items.get_mut(self.cursor).filter(|item| item.enabled) {
                    item.checked = !item.checked;
                }
            }
            ChecklistKey::SelectAll => self.check_all(true),
            ChecklistKey::SelectNone => self.check_all(false),
        }
        Outcome::Consumed
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _cx: &Cx, theme: &Theme) {
        self.draw(area, buf, theme);
    }
}

#[cfg(test)]
#[path = "checklist_tests.rs"]
mod tests;
