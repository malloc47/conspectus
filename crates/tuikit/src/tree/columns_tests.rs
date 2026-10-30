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
    assert_eq!(columns.indent(&rows[1]), INDENT);
    assert_eq!(columns.lead(&rows[1]), GUTTER_WIDTH + INDENT);
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

#[test]
fn a_look_on_part_of_a_row_leaves_the_cells_before_it_alone() {
    let theme = Theme::default();
    let names = Names::new();
    let rows = tree();
    let states = [RowState {
        mark: RowMark::Off,
        ..RowState::default()
    }];
    let columns = TreeColumns::new(&names, &rows, &TreeState::new(), &states, &theme);
    let cx = RowCtx {
        mark: RowMark::Off,
        ..cx(&theme, 60)
    };
    let plain = columns.line(&rows[0], &cx);
    let looked = columns.line_with_look(&rows[0], &cx, Col::Name, |spans| {
        crate::selection::ghost(spans, &theme);
    });
    assert_eq!(text(&looked), text(&plain), "a look changes no text");
    let arrow = &looked.spans[0];
    assert_eq!(
        arrow.style.fg,
        Some(theme.disclosure),
        "the disclosure keeps its color"
    );
    let at = |line: &Line<'_>, needle: &str| {
        line.spans
            .iter()
            .find(|span| span.content.contains(needle))
            .map(|span| span.style)
    };
    assert_eq!(
        at(&looked, "- "),
        at(&plain, "- "),
        "the mark keeps its look"
    );
    let ghost = crate::selection::ghost_style(&theme);
    assert_eq!(at(&looked, "group").and_then(|style| style.fg), ghost.fg);
    assert_eq!(at(&looked, "a note").and_then(|style| style.fg), ghost.fg);
    // Every span from the name on wears it, the gaps between them too.
    let from = looked
        .spans
        .iter()
        .position(|span| span.content.contains("group"))
        .expect("the name");
    assert!(
        looked.spans[from..]
            .iter()
            .all(|span| span.style.fg == ghost.fg),
        "{:?}",
        looked.spans
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Half {
    Name,
    Rest,
}

/// Rows whose name takes half of the whole row.
struct Halves {
    schema: ColumnSchema<Half>,
}

impl RowColumns<Item> for Halves {
    type Key = Half;

    fn schema(&self, _: &Item) -> &ColumnSchema<Half> {
        &self.schema
    }

    fn cells(&self, row: &Item, _: &RowCtx<'_>) -> Cells<Half> {
        vec![
            (Half::Name, Cell::text(row.payload, Style::default())),
            (Half::Rest, Cell::text("rest", Style::default())),
        ]
    }
}

#[test]
fn a_share_of_the_row_counts_the_cursor_gutter() {
    // A share is of the whole row, the cursor gutter the tree draws before
    // it included: a list of the same whole width that draws its own
    // gutter gives the column the same cells.
    let halves = Halves {
        schema: ColumnSchema::new([
            Column::share(Half::Name, 1, 2).gap(0),
            Column::fill(Half::Rest),
        ]),
    };
    let theme = Theme::default();
    let rows = vec![item(1, 0, false, "name")];
    let columns = TreeColumns::new(&halves, &rows, &TreeState::new(), &[], &theme);
    // A body 20 cells wide: the gutter, then 18 cells for the row.
    let line = columns.line(&rows[0], &cx(&theme, 20 - GUTTER_WIDTH));
    assert_eq!(text(&line), format!("name{}rest", " ".repeat(10 - 4)));
    let cells = halves.cells(&rows[0], &cx(&theme, 0));
    let mut list = crate::columns::Measure::new();
    list.add(&halves.schema, GUTTER_WIDTH, &cells);
    let listed = halves
        .schema
        .draw(&list, GUTTER_WIDTH, 20 - GUTTER_WIDTH, &cells, &theme);
    assert_eq!(Cell::plain(&listed), text(&line));
}
