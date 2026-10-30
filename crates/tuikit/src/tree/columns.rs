//! A tree's rows laid out from column schemas: the host gives each row's
//! cells, the tree measures them all and draws each after its indent.

use std::marker::PhantomData;

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::TreeRow;
use super::state::TreeState;
use super::view::{RowCtx, RowState};
use crate::columns::{Cell, Cells, ColumnSchema, Measure};
use crate::glyphs::marks;
use crate::theme::Theme;

/// The cells a tree indents a row by for each level of its depth.
pub const INDENT: usize = 2;

/// What a host gives a tree whose rows are laid out from column schemas:
/// the schema each row is drawn from, and its cells.
///
/// The cells are content: a row gives the same cells whatever its width,
/// cursor, or focus, because the tree measures every row once a frame
/// with a [`RowCtx`] whose `width` is 0 and which has no cursor, and the
/// schema decides what fits. A row's state (its mark, whether it is
/// pending, expanded, or a ghost) is in the context for cells that show
/// it, such as a busy mark in place of a state the row cannot vouch for.
pub trait RowColumns<R> {
    /// The key a row's cells are given under.
    type Key: Copy + Eq;

    /// The schema `row` is drawn from. Rows of one kind share a schema;
    /// schemas that share a key share that column's width.
    fn schema(&self, row: &R) -> &ColumnSchema<Self::Key>;

    /// The cells of `row`, keyed by column.
    fn cells(&self, row: &R, cx: &RowCtx<'_>) -> Cells<Self::Key>;
}

/// A tree's rows laid out from column schemas, for [`TreeView`]'s
/// producer.
///
/// [`new`](Self::new) measures every row of the tree once (rows a fold
/// hides included, so folding never moves a column) from the cells the
/// host's [`RowColumns`] gives; [`line`](Self::line) lays one row out at
/// the width its context gives: the row's indent ([`INDENT`] cells per
/// level), then its schema's cells in the cells left, by the one
/// narrow-width policy of [`crate::columns`]. The tree applies the row's
/// looks after.
///
/// [`TreeView`]: super::TreeView
///
/// ```
/// use ratatui::buffer::Buffer;
/// use ratatui::layout::Rect;
/// use ratatui::style::Style;
/// use tuikit::columns::{Cell, Cells, Column, ColumnSchema};
/// use tuikit::testkit::buffer;
/// use tuikit::theme::Theme;
/// use tuikit::tree::{self, Row, RowColumns, RowCtx, TreeColumns, TreeRow, TreeState, TreeView};
///
/// #[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// enum Col {
///     Fold,
///     Name,
///     Size,
/// }
///
/// struct Files {
///     schema: ColumnSchema<Col>,
/// }
///
/// impl RowColumns<Row<u32, (&'static str, &'static str)>> for Files {
///     type Key = Col;
///     fn schema(&self, _: &Row<u32, (&'static str, &'static str)>) -> &ColumnSchema<Col> {
///         &self.schema
///     }
///     fn cells(&self, row: &Row<u32, (&'static str, &'static str)>, cx: &RowCtx<'_>) -> Cells<Col> {
///         vec![
///             (Col::Fold, tree::disclosure(row.expandable(), cx.expanded, cx.theme)),
///             (Col::Name, Cell::text(row.payload.0, Style::default())),
///             (Col::Size, Cell::text(row.payload.1, Style::default())),
///         ]
///     }
/// }
///
/// let files = Files {
///     schema: ColumnSchema::new([
///         Column::fixed(Col::Fold, 2).gap(0),
///         Column::cap(Col::Name, 20),
///         Column::fixed(Col::Size, 4).right(),
///     ]),
/// };
/// let rows = vec![
///     Row { id: 1, depth: 0, expandable: true, payload: ("docs", "") },
///     Row { id: 2, depth: 1, expandable: false, payload: ("guide.md", "12k") },
///     Row { id: 3, depth: 0, expandable: false, payload: ("notes.txt", "3k") },
/// ];
/// let mut state = TreeState::new();
/// state.expand_id(1);
/// let theme = Theme::default();
/// let columns = TreeColumns::new(&files, &rows, &state, &[], &theme);
/// let mut buf = Buffer::empty(Rect::new(0, 0, 24, 5));
/// TreeView::new(&rows, &state, |row, cx| columns.line(row, cx), &theme)
///     .render(Rect::new(0, 0, 24, 5), &mut buf);
/// let drawn = buffer::rows(&buf);
/// assert!(drawn[2].starts_with("│    guide.md    12k "), "{drawn:?}");
/// assert!(drawn[3].starts_with("│  notes.txt    3k "), "{drawn:?}");
/// ```
pub struct TreeColumns<'a, R, P: RowColumns<R>> {
    columns: &'a P,
    measure: Measure<P::Key>,
    indent: usize,
    rows: PhantomData<fn(&R)>,
}

impl<'a, R, P> TreeColumns<'a, R, P>
where
    R: TreeRow,
    P: RowColumns<R>,
{
    /// Measures every row of `rows` from the cells `columns` gives, each
    /// with the context `state` and `states` say (index for index; a row
    /// past the end of `states` has the default state).
    pub fn new(
        columns: &'a P,
        rows: &[R],
        state: &TreeState<R::Id>,
        states: &[RowState],
        theme: &Theme,
    ) -> Self {
        let mut measure = Measure::new();
        for (index, row) in rows.iter().enumerate() {
            let row_state = states.get(index).copied().unwrap_or_default();
            let cx = RowCtx {
                selected: false,
                focused: false,
                expanded: state.is_expanded(row.id()),
                ghost: row_state.ghost,
                pending: row_state.pending,
                mark: row_state.mark,
                width: 0,
                theme,
            };
            let cells = columns.cells(row, &cx);
            measure.add(columns.schema(row), lead(row, INDENT), &cells);
        }
        Self {
            columns,
            measure,
            indent: INDENT,
            rows: PhantomData,
        }
    }

    /// What the measure pass found, for a host that still draws part of
    /// some rows itself.
    pub const fn measure(&self) -> &Measure<P::Key> {
        &self.measure
    }

    /// The cells `row`'s indent takes.
    pub fn lead(&self, row: &R) -> usize {
        lead(row, self.indent)
    }

    /// `row` laid out at the width `cx` gives: its indent, then its cells.
    pub fn line(&self, row: &R, cx: &RowCtx<'_>) -> Line<'static> {
        let lead = self.lead(row);
        let room = cx.width.saturating_sub(lead);
        let cells = self.columns.cells(row, cx);
        let mut spans = indent(lead);
        spans.extend(
            self.columns
                .schema(row)
                .draw(&self.measure, lead, room, &cells, cx.theme),
        );
        Line::from(spans)
    }

    /// `row`'s indent and its schema's cells, every one that shows drawn,
    /// then what `rest` lays out in the cells left (see
    /// [`ColumnSchema::draw_then`]): a row whose last part the host still
    /// draws itself. The tree does not cut what `rest` returns.
    pub fn line_then(
        &self,
        row: &R,
        cx: &RowCtx<'_>,
        rest: impl FnOnce(usize) -> Vec<Span<'static>>,
    ) -> Line<'static> {
        let lead = self.lead(row);
        let room = cx.width.saturating_sub(lead);
        let cells = self.columns.cells(row, cx);
        let (drawn, left) =
            self.columns
                .schema(row)
                .draw_then(&self.measure, lead, room, &cells, cx.theme);
        let mut spans = indent(lead);
        spans.extend(drawn);
        spans.extend(rest(left));
        Line::from(spans)
    }
}

/// The cells `row`'s indent takes at `per` cells a level.
fn lead<R: TreeRow>(row: &R, per: usize) -> usize {
    usize::from(row.depth()) * per
}

fn indent(cells: usize) -> Vec<Span<'static>> {
    if cells == 0 {
        Vec::new()
    } else {
        vec![Span::raw(" ".repeat(cells))]
    }
}

/// A row's disclosure mark: the expanded or the collapsed arrow in the
/// theme's `disclosure` color for a row with children, blank for one
/// without. A column of them is two cells wide (the mark and a space).
pub fn disclosure(expandable: bool, expanded: bool, theme: &Theme) -> Cell {
    if !expandable {
        return Cell::empty();
    }
    let mark = if expanded {
        marks::EXPANDED
    } else {
        marks::COLLAPSED
    };
    Cell::text(mark, Style::default().fg(theme.disclosure))
}

#[cfg(test)]
#[path = "columns_tests.rs"]
mod tests;
