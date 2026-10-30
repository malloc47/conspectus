//! Column schemas: a row of several cells declared once, measured over a
//! whole list, and given way by one rule when the list is narrow.
//!
//! This is a layer 1 foundation (house rules 13 and 14). A dense list
//! (a tree of groups and their items, a log, a table) lays each row out as
//! cells in columns, and every such list should line its columns up and
//! give way alike. Written by hand, each row kind pads to widths it works
//! out in a pre-pass of its own, types its gutters inline, and decides on
//! its own what to drop when the pane is narrow; one ends in `…`, another
//! is cut at the pane's edge with no mark. A schema says it once:
//!
//! - **Columns** ([`Column`]) are keyed by the host's key type `K` and
//!   have a width rule ([`Width`]): fixed, capped at the widest cell in the
//!   list, stepped by the room, hugging their own cell, or filling what is
//!   left. Each keeps its cells to an edge ([`Align`]), leaves a gap after
//!   it ([`GAP`] cells unless told otherwise), and may drop or shrink at a
//!   priority when the row is too narrow, or leave every row narrower than
//!   a room ([`Column::drop_below`]).
//! - **Cells** ([`Cell`]) are typed by how they cut: text at its end, in
//!   the middle, or clipped, with a tail kept whole; styled spans and
//!   chips; a list of chips with a count of the rest; the age cell; a
//!   snippet whose match stays in view. A row gives its cells as
//!   [`Cells`], keyed by column.
//! - **One measure pass** ([`Measure`]) goes over every row of the list
//!   once, rows a fold hides included, and records each key's widest cell,
//!   so a capped column is one width over the whole list and folding never
//!   moves it. Schemas that share a key share the column's width.
//! - **One narrow-width policy** ([`ColumnSchema::draw`]), below.
//!
//! # When a row is too narrow
//!
//! A row is laid out in its room, the cells left after anything the list
//! draws before the schema (a tree's indent):
//!
//! 1. A column whose [`drop_below`](Column::drop_below) room is more than
//!    the row's is left out.
//! 2. Every other column takes its width: a fixed column its cells, a
//!    capped one its widest cell in the list (none when every cell is
//!    blank), a stepped one its step, a hugging one its own cell, and a
//!    fill [`MIN_FILL`] cells.
//! 3. While the row is wider than its room, columns give way in ascending
//!    priority: one that [`shrinks`](Column::shrinks) narrows, no more
//!    than the row needs and not below its floor, and is cut by its cell's
//!    own rule; one that [`drops`](Column::drops) leaves with its gap. Of
//!    two at one priority the later goes first. A column that narrowed
//!    takes back cells a later leaving freed.
//! 4. The fill, if it still shows, takes what is left, cut with `…`. A
//!    fill with no rule of its own gives way last of all, narrowing until
//!    it is gone.
//! 5. A row that is still too wide once nothing more gives way is cut at
//!    its edge with `…`.
//!
//! The columns before the first hugging column line up: they give way
//! for the room by the widths measured over the whole list, so every row
//! drawn in the same room loses the same ones and keeps the rest in one
//! place. When one of them [absorbs the lead](Column::absorbs_lead), they
//! give way as a row with no lead would in the whole width, and only that
//! column is narrower on a deeper row, so rows at every depth lose the
//! same ones. The hugging column and the columns after it follow each row's
//! own cells and give way row by row. A blank cell of a lined-up column is
//! padded while a later cell on its row shows; a row ends at its last cell
//! that is not blank, and draws no gap after it. A row's last cell needs
//! only its own text, not its column's pad, and a [ragged](Column::ragged)
//! cell that ends its row runs on into the room left.
//!
//! ```
//! use ratatui::style::Style;
//! use tuikit::columns::{Cell, Cells, Column, ColumnSchema, Measure};
//! use tuikit::theme::Theme;
//!
//! #[derive(Clone, Copy, Debug, PartialEq, Eq)]
//! enum Col {
//!     Name,
//!     Size,
//!     Note,
//! }
//!
//! // A name padded to the widest in the list, a size kept to the right
//! // that leaves first when the row is narrow, and a note in the rest.
//! let schema = ColumnSchema::new([
//!     Column::cap(Col::Name, 16),
//!     Column::fixed(Col::Size, 5).right().drops(1),
//!     Column::fill(Col::Note).drops(0),
//! ]);
//! let plain = Style::default();
//! let row = |name: &str, size: &str, note: &str| -> Cells<Col> {
//!     vec![
//!         (Col::Name, Cell::text(name, plain)),
//!         (Col::Size, Cell::text(size, plain)),
//!         (Col::Note, Cell::text(note, plain)),
//!     ]
//! };
//! let rows = [row("alpha", "12k", "first draft"), row("beta-two", "3k", "")];
//! let mut measure = Measure::new();
//! for cells in &rows {
//!     measure.add(&schema, 0, cells);
//! }
//! let theme = Theme::default();
//! let draw = |cells: &Cells<Col>, room: usize| Cell::plain(&schema.draw(&measure, 0, room, cells, &theme));
//!
//! assert_eq!(draw(&rows[0], 40), "alpha       12k  first draft");
//! assert_eq!(draw(&rows[1], 40), "beta-two     3k");
//! // Too narrow for the note's seven cells: the note leaves first, then
//! // the size. A lined-up cell keeps its column's width to the end.
//! assert_eq!(draw(&rows[0], 20), "alpha       12k");
//! assert_eq!(draw(&rows[0], 12), "alpha   ");
//! ```

mod cell;
mod draw;
mod measure;
mod schema;

pub use cell::{Cell, Cut};
pub use draw::Layout;
pub use measure::Measure;
pub use schema::{Align, Column, ColumnSchema, Width};

/// One row's cells, each under the key of the column that shows it. A key
/// the row gives no cell for is blank.
pub type Cells<K> = Vec<(K, Cell)>;

/// The blank cells a column leaves after it unless told otherwise.
pub const GAP: usize = 2;

/// The fewest cells a fill shows in, unless its whole text takes fewer:
/// one that would get fewer leaves its row (or narrows, when it has no
/// rule of its own and nothing else can give way). Seven cells show six of
/// a text's characters and the ellipsis mark, enough to tell two rows
/// apart; fewer is mostly the mark.
pub const MIN_FILL: usize = 7;

#[cfg(test)]
#[path = "columns_tests.rs"]
mod tests;
