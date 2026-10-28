//! Help: the page that lists what keys do, and the generator that writes
//! it from a key table.
//!
//! This is a layer 2 convention (house rules 8 and 12). A help page is a
//! reader in a modal frame: section headings, a key column in the accent
//! color with what each key does wrapped beside it, legends for the marks
//! a screen draws, a scrollbar when the page is taller than its frame, and
//! a footer generated from the key table that scrolls and closes it. Every
//! help page of a program draws through [`HelpPage`], so the pages share
//! one frame, one column grid, and one footer.
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
//!   when a key has no description.
//! - **The page.** [`HelpPage`] lays the sections out in cells for the
//!   width its frame gives, through [`text`](crate::text), draws them in a
//!   [`ModalFrame`](crate::frame::ModalFrame) at [`HelpPage::SIZE`] with a
//!   [`ScrollGutter`](crate::scroll::ScrollGutter) when they overflow, and
//!   keeps the footer on the frame's last rows.
//! - **Scrolling.** [`HelpScroll`] is where the page is. A key moves it;
//!   the draw brings it back inside the page and returns the window it
//!   showed, which the scroll keeps, so a page key moves by the rows on
//!   screen and no key presses pile up past the end.
//!
//! ```
//! use ratatui::buffer::Buffer;
//! use ratatui::crossterm::event::KeyCode;
//! use ratatui::layout::Rect;
//! use tuikit::help::{HelpAction, HelpEntry, HelpPage, HelpScroll, sections_for, undescribed};
//! use tuikit::keys::{KeyRow, PAGER_KEYS, alias, ch, ctrl, key};
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
//! // Draw the page with a footer from the keys that scroll and close it,
//! // and keep the window it showed.
//! let theme = Theme::default();
//! let area = Rect::new(0, 0, 80, 24);
//! let mut buf = Buffer::empty(area);
//! let mut scroll = HelpScroll::new();
//! let window = HelpPage::new(&theme, &sections)
//!     .footer(PAGER_KEYS)
//!     .scrolled(&scroll)
//!     .render(area, &mut buf);
//! scroll.keep(window);
//! assert!(!window.overflows(), "a short page needs no scrollbar");
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

pub use content::{HelpAction, HelpEntry, HelpRow, HelpSection, sections_for, undescribed};
pub use page::{HelpPage, HelpScroll};

#[cfg(test)]
#[path = "help_tests.rs"]
mod tests;
