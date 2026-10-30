//! Text: measuring, cutting, padding, wrapping, and marking strings in
//! terminal cells.
//!
//! This is a layer 1 foundation (house rule 14). A terminal gives a CJK
//! character or an emoji two cells, a combining accent none, and an
//! emoji written with a variation selector the two cells of the picture
//! it draws. Code that counts `char`s instead cuts a row a cell late or
//! early, and a column after it no longer lines up. Every function here
//! measures cells and moves in whole graphemes (the letter and its
//! accent, the emoji and its selector), so no cut, wrap, or highlight
//! separates them.
//!
//! - **Measure**: [`width`] and [`spans_width`].
//! - **Cut and pad**: [`truncate_end`] for labels, [`truncate_middle`]
//!   for paths and identifiers, [`clip`] and [`clip_start`] for a hard
//!   cut with no mark (keeping the start or the end),
//!   [`pad`] and [`pad_start`] to fill a column, [`fit`] to make a
//!   column exactly so wide, and [`fit_spans`] to cut a styled row. Every
//!   cut that loses text ends in [`marks::ELLIPSIS`](crate::glyphs::marks::ELLIPSIS),
//!   inside the budget.
//! - **Wrap**: [`wrap`] (words), [`wrap_chars`] (any grapheme, as a
//!   terminal wraps), [`wrap_at`] (only at a separator, for lists of key
//!   hints), and the styled [`wrap_line`] and [`wrap_line_chars`]. A
//!   surface that needs its row count (a scroll range, a scrollbar, a
//!   height budget) wraps with these and draws the rows unwrapped, so
//!   the count is the count drawn.
//! - **Snippets**: [`fit_snippet`] cuts a one-line excerpt around a match
//!   so the match stays on screen.
//! - **Give way**: [`elide`] fits the pieces of a line (the steps of a
//!   path, the names of a strip, the hints of a status line) into a room:
//!   pieces give way by rank, each run that gave way leaves one mark, in
//!   its gap or at the end, and a piece that cannot give way is cut last.
//! - **Find and paint**: [`find_ci`] and [`find_all_ci`] find matches
//!   without case for ASCII; [`paint`] restyles them, the one match
//!   painter.
//! - **Escapes**: [`strip_ansi`] drops the terminal escape sequences of
//!   captured output or ANSI-written frames, so the text can be measured,
//!   shown plainly, or compared.
//!
//! ```
//! use tuikit::text::{fit, truncate_end, truncate_middle, width, wrap};
//!
//! assert_eq!(width("東京"), 4);
//! assert_eq!(truncate_end("a long label", 8), "a long …");
//! assert_eq!(truncate_middle("/srv/data/archive/report.txt", 16), "/srv/dat…ort.txt");
//! assert_eq!(fit("id", 4), "id  ");
//! assert_eq!(wrap("one two three", 7), ["one two", "three"]);
//! ```
//!
//! # Wrapping rules
//!
//! Every word wrapper lays out rows the same way, styled or not:
//!
//! - A row breaks at the last space that fits. A word that ends exactly
//!   at the edge stays on its row.
//! - Spaces before a break stay on their row when they fit; spaces that
//!   would run past the edge are dropped, and a continuation row never
//!   starts with one. The first row keeps its leading indent.
//! - A word wider than a row is broken between graphemes, and the rest of
//!   it carries on as ordinary text.
//! - No grapheme is split, and no row is wider than the width. A
//!   character wider than the whole row (a two-cell character in a row of
//!   one) cannot be drawn, so the ellipsis mark stands in for it.
//! - A width of 0 is treated as 1.
//!
//! These are the rules ratatui's word wrapper follows when it keeps
//! whitespace (`Wrap { trim: false }`), so a pre-wrapped surface reads
//! like a wrapped paragraph.

mod cut;
mod elide;
mod escape;
mod find;
mod measure;
mod snippet;
mod wrap;

pub use cut::{clip, clip_start, fit, fit_spans, pad, pad_start, truncate_end, truncate_middle};
pub use elide::{Elided, Item, MarkAt, Piece, elide};
pub use escape::strip_ansi;
pub use find::{FindAll, find_all_ci, find_ci, paint, paint_matches};
pub use measure::{spans_width, width};
// The text field windows a line by the same graphemes every cut uses.
pub(crate) use measure::{Cluster, clusters};
pub use snippet::{Snippet, fit_snippet};
pub use wrap::{wrap, wrap_at, wrap_chars, wrap_line, wrap_line_chars};

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
