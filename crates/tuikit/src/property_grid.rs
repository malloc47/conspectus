//! Property grids: the fields of one thing, the links that lead away from
//! it, and the sections that fold some of them away.
//!
//! This is a layer 1 control. A detail pane lists what it knows about
//! the thing it shows as rows of a grid, each row one of three kinds:
//!
//! - **[`PropertyRow`]**: a label in a fixed label column, then the
//!   value. The value is text or styled spans; it can be cut to a number
//!   of cells, at its end or in its middle ([`Cut`]), and a cut value
//!   says so with a hint (`(truncated · o)`, naming the key the host
//!   binds to show it whole). A row with no value (a stand-in such as the
//!   no-value mark) recedes. An optional glyph sits between the label and
//!   the value, and an annotation that needs attention follows it.
//! - **[`LinkRow`]**: where a link leads, `verb  glyph  label`: an
//!   indent, a verb in a fixed verb column, a one-cell glyph for the kind
//!   of thing at the far end, and a label cut in the middle to the room
//!   left, so the row never wraps. Optional meta follows the label, which
//!   leaves it room down to [`MIN_LINK_LABEL`] cells of its own; past
//!   that the meta runs on, for a host that wraps its lines to carry to
//!   the next row. Its [`Tone`] says how much the link holds:
//!   primary, quiet (an alternative), alert (a conflict: led by the
//!   warning mark, so the signal survives a terminal with no color), or
//!   inert (a link with no far end: the whole row recedes and leads
//!   nowhere). [`Tones`] maps the four onto styles, from the theme's
//!   tokens or the host's own roles.
//! - **[`DisclosureRow`]**: a section header the cursor can stand on,
//!   with the disclosure mark (the same mark and color as a tree's
//!   folding rows), a label, a count, and tallies of the rows that carry
//!   a mark (`(3 · 1 ▲)`).
//!
//! A [`PropertyGrid`] holds the rows in order, with lines that are no
//! row (a rule or a heading the host draws) between them, and draws
//! every row through [`selection::row`](crate::selection::row): the
//! cursor gutter, and the selection pair over the whole row, focused or
//! not. The cursor is an index over the rows alone.
//!
//! A row never acts on its own. [`PropertyGrid::activate`] (the host's
//! `Enter`) and [`PropertyGrid::inspect`] (the host's inspect key) say
//! what the row under the cursor asks for, an [`Intent`]: copy a value,
//! inspect it whole, open or close a section, follow a link. The host
//! does the effect: it writes the copy to its clipboard sink, opens the
//! whole value in a reader (a [`Pager`](crate::pager::Pager)), folds the
//! section, or moves to the link's far end.
//!
//! ```
//! use ratatui::text::Span;
//! use tuikit::columns::Cut;
//! use tuikit::glyphs::marks;
//! use tuikit::property_grid::{
//!     DisclosureRow, Intent, LinkRow, PropertyGrid, PropertyRow, Tone,
//! };
//! use tuikit::theme::Theme;
//!
//! let theme = Theme::default();
//! let grid = PropertyGrid::new()
//!     .inspect_key("o")
//!     .row(PropertyRow::new("name", "build cache"))
//!     .row(PropertyRow::new("note", "a long note that will not fit").cut(Cut::End, 12))
//!     .row(PropertyRow::new("owner", marks::NO_VALUE).placeholder())
//!     .row(LinkRow::new("stored in", "/srv/cache").glyph(Span::raw("#")))
//!     .row(DisclosureRow::new("Other", false).count(2))
//!     .row(LinkRow::new("replaces", "old cache").indent(4).tone(Tone::Quiet));
//!
//! let drawn = grid.lines(60, Some(1), true, &theme);
//! let text: Vec<String> = drawn.lines.iter().map(ToString::to_string).collect();
//! assert_eq!(text[1].trim_end(), "► note          a long note…  (truncated · o)");
//! assert_eq!(text[3].trim_end(), "  stored in              # /srv/cache");
//! assert_eq!(text[4].trim_end(), "  ▶ Other  (2)");
//! assert_eq!(drawn.cursor_line, Some(1));
//!
//! assert_eq!(
//!     grid.inspect(1),
//!     Some(Intent::Inspect {
//!         label: "note".into(),
//!         text: "a long note that will not fit".into(),
//!     })
//! );
//! assert_eq!(grid.activate(2), None, "a placeholder has nothing to copy");
//! assert_eq!(grid.activate(3), Some(Intent::Follow));
//! assert_eq!(grid.activate(4), Some(Intent::Toggle));
//! ```
//!
//! # What the host keeps
//!
//! What each row says (which fields a thing has, which verb names a
//! link, which tone a link takes, which glyph marks its far end), the
//! cursor and which rows a closed section hides, the keys, and every
//! effect. The grid draws rows and reports intents; it reads no host
//! state and keeps none of its own.

mod disclosure;
mod grid;
mod link;
mod property;

pub use crate::columns::Cut;
pub use disclosure::DisclosureRow;
pub use grid::{GridLines, Intent, PropertyGrid, Row};
pub use link::{LinkRow, Tone, Tones};
pub use property::PropertyRow;

/// The cells of a grid's label column unless it is told otherwise
/// ([`PropertyGrid::label_width`]). A label as wide or wider keeps one
/// blank cell before its value.
pub const LABEL_WIDTH: usize = 14;

/// The cells of a grid's verb column unless it is told otherwise
/// ([`PropertyGrid::verb_width`]).
pub const VERB_WIDTH: usize = 22;

/// The fewest cells a link row's label is cut to so its meta fits beside
/// it. With less room the label takes the room, and the meta runs past
/// the row's width.
pub const MIN_LINK_LABEL: usize = 8;

#[cfg(test)]
#[path = "property_grid_tests.rs"]
mod tests;
