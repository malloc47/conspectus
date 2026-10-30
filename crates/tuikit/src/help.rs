//! Help: the page that lists what keys do, and the generator that writes
//! it from a key table.
//!
//! This is a layer 2 convention (house rules 8 and 12). A help page is
//! the content of a framed [`Pager`](crate::pager::Pager): section
//! headings, a key column in the accent color with what each key does
//! wrapped beside it, and legends for the marks a screen draws; the pager
//! scrolls it, draws a scrollbar when the page is taller than its frame,
//! and generates the footer from the key table that scrolls and closes
//! it. Every help page of a program is a [`HelpPage`] in a pager, so the
//! pages share one frame, one column grid, one footer, and one way of
//! scrolling with every other reader.
//!
//! - **Content.** A page is a list of [`HelpSection`]s: a heading and its
//!   [`HelpRow`]s, each a row of keys and what they do or a legend entry
//!   (a mark, its name, and what it means). A host writes sections by
//!   hand where its keys need prose, and generates them where a key table
//!   says it all.
//! - **Generated from a key table.** [`sections_for`] reads a table of
//!   [`KeyRow`](crate::keys::KeyRow)s over an action enum that implements
//!   [`HelpAction`]: the enum lists its actions in page order and gives
//!   each a [`HelpEntry`] (its section and what it does). Each listed
//!   action the table binds becomes one row with every key bound to it;
//!   rows group under their sections; a row whose keys cannot act now is
//!   dimmed. The page cannot disagree with the table, and
//!   [`undescribed`] finds a key the page would leave out, so a test fails
//!   when a key has no description. [`sections_described`] is the same
//!   page in the host's words, for a table over a framework enum.
//! - **The page.** [`HelpPage`] lays the sections out in cells for the
//!   width its pager gives, through [`text`](crate::text), and is a
//!   [`PagerContent`](crate::pager::PagerContent);
//!   [`HelpPage::pager`] reads it in a pager framed at
//!   [`HelpPage::SIZE`] that resolves the table the page closes with.
//! - **Scrolling.** The pager's: one window clamped on every draw, so a
//!   page key moves by the rows on screen, no key presses pile up past
//!   the end, and `G` shows the last page.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::KeyCode;
//! use ratatui::layout::Rect;
//! use tuikit::help::{HelpAction, HelpEntry, HelpPage, sections_for, undescribed};
//! use tuikit::keys::{KeyRow, PAGER_KEYS, alias, ch, ctrl, key};
//! use tuikit::overlay::Outcome;
//! use tuikit::testkit::keys::{press, typed};
//! use tuikit::theme::Theme;
//!
//! /// What a document reader's keys do.
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! enum Reader {
//!     Down,
//!     Up,
//!     Leave,
//! }
//!
//! impl HelpAction for Reader {
//!     const LISTED: &'static [Self] = &[Reader::Down, Reader::Up, Reader::Leave];
//!
//!     fn help(self) -> HelpEntry {
//!         match self {
//!             Reader::Down => HelpEntry::new("Moving", "Scroll one line down"),
//!             Reader::Up => HelpEntry::new("Moving", "Scroll one line up"),
//!             Reader::Leave => HelpEntry::new("Leaving", "Close the reader"),
//!         }
//!     }
//! }
//!
//! const READER_KEYS: &[KeyRow<Reader>] = &[
//!     alias(ctrl('c'), Reader::Leave),
//!     alias(ch('j'), Reader::Down),
//!     alias(key(KeyCode::Down), Reader::Down),
//!     alias(ch('k'), Reader::Up),
//!     alias(ch('q'), Reader::Leave),
//! ];
//!
//! // One row per action, every key bound to it, plain keys first.
//! let sections = sections_for(READER_KEYS);
//! assert_eq!(sections.len(), 2);
//! assert_eq!(sections[1].title, "Leaving");
//! assert!(matches!(
//!     &sections[1].rows[0],
//!     tuikit::help::HelpRow::Keys { keys, .. } if keys == "q / Ctrl-C"
//! ));
//! // Every key the table binds is described.
//! assert!(undescribed(READER_KEYS).is_empty());
//!
//! // Read the page in a pager whose table scrolls and closes it, which
//! // also generates its footer.
//! let theme = Theme::default();
//! let area = Rect::new(0, 0, 80, 24);
//! let mut buf = Buffer::empty(area);
//! let mut page = HelpPage::<()>::new(sections).pager(PAGER_KEYS);
//! page.draw(area, &mut buf, &theme);
//! assert!(!page.window().overflows(), "a short page needs no scrollbar");
//! assert_eq!(page.handle_key(typed('G')), Outcome::Consumed);
//! assert_eq!(page.handle_key(press(KeyCode::Esc)), Outcome::Close);
//! ```
//!
//! # What a host keeps
//!
//! What each action does and where it is listed (its [`HelpAction`]
//! impl), the prose of rows it writes by hand and the marks its legends
//! explain, which key table scrolls and closes its pages, and which
//! actions are available now. A later step generates a program-wide
//! keymap's page from the scoped keymap's own rows.

mod content;
mod page;

pub use content::{
    HelpAction, HelpEntry, HelpRow, HelpSection, sections_described, sections_for, undescribed,
};
pub use page::HelpPage;

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;
