use ratatui::style::Style;

use super::*;
use crate::columns::{Column, ColumnSchema};
use crate::tree::{Row, RowMark};

type Item = Row<u32, &'static str>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Col {
    Fold,
    Mark,
    Name,
    Note,
}

/// Rows laid out as a disclosure, a mark that only marked rows have, the
/// name padded to the widest in the tree, and a note after it.
struct Names {
    schema: ColumnSchema<Col>,
}

impl Names {
    fn new() -> Self {
        Self {
            schema: ColumnSchema::new([
                Column::fixed(Col::Fold, 2).gap(0),
                Column::cap(Col::Mark, 2).gap(0),
                Column::cap(Col::Name, 30),
                Column::fill(Col::Note).drops(0),
            ]),
        }
    }

    /// The disclosure and the mark only: a row the host goes on to draw.
    fn prefix() -> Self {
        Self {
            schema: ColumnSchema::new([
                Column::fixed(Col::Fold, 2).gap(0),
                Column::cap(Col::Mark, 2).gap(0),
            ]),
        }
    }
}

impl RowColumns<Item> for Names {
    type Key = Col;

    fn schema(&self, _: &Item) -> &ColumnSchema<Col> {
        &self.schema
    }

    fn cells(&self, row: &Item, cx: &RowCtx<'_>) -> Cells<Col> {
        let mark = match cx.mark {
            RowMark::None => Cell::empty(),
            RowMark::On => Cell::text("+ ", Style::default()),
            RowMark::Off => Cell::text("- ", Style::default()),
        };
        vec![
            (Col::Fold, disclosure(row.expandable, cx.expanded, cx.theme)),
            (Col::Mark, mark),
            (Col::Name, Cell::text(row.payload, Style::default())),
            (Col::Note, Cell::text("a note", Style::default())),
        ]
    }
}

const fn item(id: u32, depth: u16, expandable: bool, name: &'static str) -> Item {
    Row {
        id,
        depth,
        expandable,
        payload: name,
    }
}

fn tree() -> Vec<Item> {
    vec![
        item(1, 0, true, "group"),
        item(2, 1, false, "a-much-longer-name"),
        item(3, 0, false, "leaf"),
    ]
}

fn cx(theme: &Theme, width: usize) -> RowCtx<'_> {
    RowCtx {
        selected: false,
        focused: true,
        expanded: false,
        ghost: false,
        pending: false,
        mark: RowMark::None,
        width,
        theme,
    }
}

fn text(line: &Line<'_>) -> String {
    Cell::plain(&line.spans)
}

#[test]
fn the_measure_covers_rows_a_fold_hides() {
    let theme = Theme::default();
    let names = Names::new();
    let rows = tree();
    // Nothing expanded: the long name is hidden, and still widens the column.
    let state = TreeState::new();
    let columns = TreeColumns::new(&names, &rows, &state, &[], &theme);
    assert_eq!(columns.measure().widest(Col::Name), 18);
    assert_eq!(
        text(&columns.line(&rows[2], &cx(&theme, 40))),
        format!("  leaf{}a note", " ".repeat(16))
    );
}

#[test]
fn a_row_is_indented_by_its_depth_and_laid_out_in_the_cells_left() {
    let theme = Theme::default();
    let names = Names::new();
    let rows = tree();
    let columns = TreeColumns::new(&names, &rows, &TreeState::new(), &[], &theme);
    assert_eq!(columns.lead(&rows[1]), INDENT);
    let line = columns.line(&rows[1], &cx(&theme, 26));
    assert_eq!(text(&line), "    a-much-longer-name");
    assert!(crate::text::spans_width(&line.spans) <= 26);
}

#[test]
fn a_mark_on_any_row_is_reserved_on_every_row() {
    let theme = Theme::default();
    let names = Names::new();
    let rows = tree();
    let states = [
        RowState::default(),
        RowState::default(),
        RowState {
            mark: RowMark::On,
            ..RowState::default()
        },
    ];
    let columns = TreeColumns::new(&names, &rows, &TreeState::new(), &states, &theme);
    let unmarked = columns.line(&rows[0], &cx(&theme, 60));
    assert!(text(&unmarked).starts_with(&format!("{}   group", crate::glyphs::marks::COLLAPSED)));
    let marked = columns.line(
        &rows[2],
        &RowCtx {
            mark: RowMark::On,
            ..cx(&theme, 60)
        },
    );
    assert!(text(&marked).starts_with("  + leaf"), "{}", text(&marked));
}

#[test]
fn a_host_drawn_rest_follows_the_cells_left() {
    let theme = Theme::default();
    let names = Names::prefix();
    let rows = tree();
    let states = [RowState {
        mark: RowMark::Off,
        ..RowState::default()
    }];
    let columns = TreeColumns::new(&names, &rows, &TreeState::new(), &states, &theme);
    let mut given = None;
    let line = columns.line_then(&rows[1], &cx(&theme, 50), |left| {
        given = Some(left);
        vec![Span::raw("rest")]
    });
    // The indent, the blank disclosure, and the blank mark the first
    // row's reserves, then the rest.
    assert_eq!(text(&line), "      rest");
    assert_eq!(given, Some(50 - INDENT - 4));
}

#[test]
fn the_disclosure_cell_is_an_arrow_for_a_row_with_children() {
    let theme = Theme::default();
    assert!(disclosure(false, false, &theme).is_empty());
    let open = disclosure(true, true, &theme);
    assert_eq!(
        Cell::plain(&open.render(2, &theme)),
        crate::glyphs::marks::EXPANDED
    );
    let closed = disclosure(true, false, &theme).render(2, &theme);
    assert_eq!(closed[0].style.fg, Some(theme.disclosure));
}
