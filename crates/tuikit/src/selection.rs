//! Selection: how a list shows its cursor row, and what its cursor does
//! at the ends of the list.
//!
//! This is a layer 1 foundation (house rules 1, 2, and 10). Every list a
//! program draws (a menu, a picker, a checklist, a settings list, a
//! form's fields, a log, a tree, a property list) has a cursor row, and
//! every one of them shows it the same way here:
//!
//! - **The marker.** Each row starts with a 2-cell gutter
//!   ([`GUTTER_WIDTH`]): [`marks::POINTER`](crate::glyphs::marks::POINTER)
//!   and a space on the cursor row, two spaces on every other row
//!   ([`gutter`]). The text after the gutter never moves as the cursor
//!   does, and the marker alone tells the cursor row apart when the
//!   terminal draws no color or no reverse video.
//! - **The highlight.** The cursor row takes the theme's selection pair
//!   from its gutter to its last span ([`highlight`]): `selection_active`
//!   while the list has focus, `selection_inactive` while it does not, so
//!   an unfocused list still shows where its cursor is. A filled span (a
//!   chip, an input field's fill: anything with a background or drawn
//!   reversed) keeps its look, so it still reads as what it is inside
//!   the highlight.
//! - **A row.** [`row`] puts the gutter before a row's spans and
//!   highlights them: the one call a list makes for each row it draws.
//!   [`Cursor`] says whether the cursor is on the row and whether its
//!   list has focus.
//! - **Ghost and pending.** A row that stands for something gone or not
//!   started yet wears the ghost look ([`ghost`], [`ghost_style`]); a row
//!   waiting for a change to land recedes ([`pending`]). Either is
//!   applied before the highlight, so a selected ghost row is still a
//!   ghost.
//! - **The ends.** [`Ends`] steps a cursor by the list's kind: short
//!   choice lists wrap from the last row to the first, scrollable
//!   content stops at its first and last rows (house rule 2). It is the
//!   [`scroll`](crate::scroll) module's two steps, named by the rule
//!   that picks between them.
//!
//! ```
//! use ratatui::style::Modifier;
//! use ratatui::text::Span;
//! use tuikit::glyphs::marks;
//! use tuikit::selection::{self, Cursor, Ends};
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//! let items = ["Open", "Rename", "Close"];
//! let cursor = Ends::Wrap.step(0, items.len(), -1);
//! assert_eq!(cursor, 2, "a menu wraps from its first row to its last");
//!
//! let lines: Vec<_> = items
//!     .iter()
//!     .enumerate()
//!     .map(|(index, item)| {
//!         selection::row([Span::raw(*item)], Cursor::of(index == cursor, true), &theme)
//!     })
//!     .collect();
//! assert_eq!(lines[0].spans[0].content, "  ");
//! assert_eq!(lines[2].spans[0].content, format!("{} ", marks::POINTER));
//! assert!(lines[2].spans[1].style.add_modifier.contains(Modifier::REVERSED));
//!
//! // Unfocused, the row keeps its marker and takes the inactive look.
//! let unfocused = selection::row([Span::raw("Close")], Cursor::of(true, false), &theme);
//! assert_eq!(unfocused.spans[1].style.add_modifier, theme.selection_inactive);
//! ```
//!
//! # What a list keeps
//!
//! The list keeps its own cursor (an index, or a key it finds again
//! after a rebuild), decides what a row says, and passes each row's
//! spans to [`row`]. The scroll window that keeps the cursor in view is
//! the [`scroll`](crate::scroll) module's. A value that cycles in place
//! (a toggle's states, a choice inside one form field) has no cursor row
//! and steps with [`scroll::wrap_step`](crate::scroll::wrap_step)
//! directly.

mod ends;
mod look;

pub use ends::Ends;
pub use look::{Cursor, GUTTER_WIDTH, ghost, ghost_style, gutter, highlight, pending, row};

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
