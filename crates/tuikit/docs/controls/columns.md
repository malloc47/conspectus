# `ColumnSchema`

| | |
| --- | --- |
| Module | `tuikit::columns`, with the tree's producer `tuikit::tree::{RowColumns, TreeColumns}` |
| Layer | 1 (component) |
| Type parameters | `K`, the host's column key (`Copy + Eq`, usually a small enum); for a tree, `R`, the host's row type, and the host's `RowColumns<R>` implementation, which names each row's schema and gives its cells |
| Cargo feature | none |
| Outcomes | none: drawing returns a row's spans, and `draw_then` also the cells left after them |

## Purpose

A dense list lays each row out as cells in columns: a mark, a name padded
so the next column lines up, a chip, an age, a note that takes the rest.
Written by hand, every row kind pads to widths a pre-pass of its own works
out, types its gutters inline, keeps a literal width per cell, and decides
on its own what to drop when the pane is narrow, so one kind ends in `…`
and another is cut at the pane's edge with no mark. A column schema states
the row once: keyed columns with a width rule, an edge, a gap, and how each
gives way; typed cells that each cut one way; one measure pass over the
whole list; and one narrow-width policy every row follows.

The host keeps what the columns are for and what each cell shows: which
schema a kind of row uses, the cells' text, styles, and chips, its own
widths and priorities, and its glyphs. The schema keeps where each cell
sits, how wide it is, how it is padded and cut, and what gives way.

## Usage example

A list of files whose name column is as wide as the widest name, whose
size keeps to the right and leaves first when the list is narrow, and
whose note takes the rest of the row:

```rust
use ratatui::style::Style;
use tuikit::columns::{Cell, Cells, Column, ColumnSchema, Measure};
use tuikit::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Col {
    Name,
    Size,
    Note,
}

let schema = ColumnSchema::new([
    Column::cap(Col::Name, 16),
    Column::fixed(Col::Size, 5).right().drops(1),
    Column::fill(Col::Note).drops(0),
]);
let plain = Style::default();
let row = |name: &str, size: &str, note: &str| -> Cells<Col> {
    vec![
        (Col::Name, Cell::text(name, plain)),
        (Col::Size, Cell::text(size, plain)),
        (Col::Note, Cell::text(note, plain)),
    ]
};
let rows = [row("alpha", "12k", "first draft"), row("beta-two", "3k", "")];

// One measure pass over every row, then each row drawn from it.
let mut measure = Measure::new();
for cells in &rows {
    measure.add(&schema, 0, cells);
}
let theme = Theme::default();
let draw = |cells: &Cells<Col>, room: usize| Cell::plain(&schema.draw(&measure, 0, room, cells, &theme));
assert_eq!(draw(&rows[0], 40), "alpha       12k  first draft");
assert_eq!(draw(&rows[1], 40), "beta-two     3k");
assert_eq!(draw(&rows[0], 20), "alpha       12k");
```

A tree draws its rows from schemas through `TreeColumns`, the producer
`TreeView` takes. The host implements `RowColumns` on what it lays rows out
with:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use tuikit::columns::{Cell, Cells, Column, ColumnSchema};
use tuikit::theme::Theme;
use tuikit::tree::{self, Row, RowColumns, RowCtx, TreeColumns, TreeRow, TreeState, TreeView};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Col {
    Fold,
    Name,
    Size,
}

type File = Row<u32, (&'static str, &'static str)>;

struct Files {
    schema: ColumnSchema<Col>,
}

impl RowColumns<File> for Files {
    type Key = Col;

    fn schema(&self, _: &File) -> &ColumnSchema<Col> {
        &self.schema
    }

    fn cells(&self, row: &File, cx: &RowCtx<'_>) -> Cells<Col> {
        vec![
            (Col::Fold, tree::disclosure(row.expandable(), cx.expanded, cx.theme)),
            (Col::Name, Cell::text(row.payload.0, Style::default())),
            (Col::Size, Cell::text(row.payload.1, Style::default())),
        ]
    }
}

let files = Files {
    schema: ColumnSchema::new([
        Column::fixed(Col::Fold, 2).gap(0),
        Column::cap(Col::Name, 20),
        Column::fixed(Col::Size, 4).right(),
    ]),
};
let rows: Vec<File> = vec![
    Row { id: 1, depth: 0, expandable: true, payload: ("docs", "") },
    Row { id: 2, depth: 1, expandable: false, payload: ("guide.md", "12k") },
];
let mut state = TreeState::new();
state.expand_id(1);
let theme = Theme::default();
// Every row measured once a frame, folded rows included.
let columns = TreeColumns::new(&files, &rows, &state, &[], &theme);
let area = Rect::new(0, 0, 24, 4);
let mut buf = Buffer::empty(area);
TreeView::new(&rows, &state, |row, cx| columns.line(row, cx), &theme).render(area, &mut buf);
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `Column::fixed`, `cap`, `steps`, `hug`, `fill` | a column by its width rule (`Width`) | `Fixed(n)` always; `Cap(n)` the widest cell in the list, at most `n` (a column whose cells are all blank takes nothing); `Steps` by the room left for it; `Hug(n)` its own cell, unpadded, after which the row flows; `Fill` the rest |
| `.gap(n)` | blank cells after the column | `GAP` (2) unless told; never drawn after a row's last cell |
| `.right()` | keep cells to the column's end | counts, numbers |
| `.drops(p)`, `.shrinks(p, floor)` | how the column gives way, at priority `p` (lowest first) | a column may narrow at one priority and leave at a later one |
| `.drop_below(room)` | leave the column out of every row narrower than `room` | whatever else would fit |
| `.ragged()` | only cells something follows widen the column; a cell that ends its row runs on into the room | a description that spaces a count |
| `.absorbs_lead()` | measure from the row's start, so the column ends in one place at every depth | a deeper row's cell is narrower by its indent |
| `ColumnSchema::new(columns)` | a row's columns in order | schemas that share a key share that column's width |
| `Cell::text(text, style)`, `.middle()`, `.clip()`, `.tail(spans)` | text cut at its end (the default), in the middle, or with no mark, with spans kept whole after it | `Cell::secondary` is muted text cut in the middle |
| `Cell::spans`, `Cell::chip` | spans the host styled, cut at the end as one | a filled chip keeps its fill |
| `Cell::chips(chips, rest)` | as many whole chips as fit, then ` +N` | the first chip is cut only when no chip fits |
| `Cell::age(label, recency)` | the framework's age cell at the column's width | `age::cell` |
| `Cell::snippet(text, highlight, style, matched)`, `.dropping(chars)` | an excerpt cut around its match, the match painted | `text::fit_snippet` and `text::paint` |
| `Measure::new`, `add(schema, lead, cells)` | measure one row of the list | cells under keys the row's schema lacks are measured too, for a host that draws part of a row itself |
| `Measure::widest`, `widest_with_lead`, `any` | what the pass found under a key | |
| `ColumnSchema::layout(measure, lead, room)` | which lined-up columns show at a room, and how wide | the decision every row at that room shares |
| `ColumnSchema::draw(measure, lead, room, cells, theme)` | one row's spans, never wider than the room | the policy below |
| `ColumnSchema::draw_then(...)` | the columns drawn, padded, and the cells left | for a row whose last part the host draws |
| `tree::RowColumns`, `TreeColumns::new`, `line`, `line_then`, `measure`, `lead` | a tree's rows from schemas | indents by `tree::INDENT` cells a level |
| `tree::disclosure(expandable, expanded, theme)` | the disclosure arrow as a cell | blank for a row without children |

### When a row is too narrow

1. A column whose `drop_below` room is more than the row's is left out.
2. Every other column takes its width: fixed, capped at its widest cell
   (less the row's lead when it absorbs it), stepped, hugging its cell,
   or a fill at `MIN_FILL` (7) cells.
3. While the row is wider than its room, columns give way in ascending
   priority (the later of two at one priority first, a narrowing before a
   leaving): a shrinking column narrows no more than needed and not below
   its floor (or its whole text, when that is narrower), cut by its cell's
   rule; a dropping one leaves with its gap. A column that narrowed takes
   back what a later leaving freed.
4. The fill, if it still shows, takes what is left, cut with `…`. It shows
   only with `MIN_FILL` cells or its whole text. A fill with no rule of its
   own gives way last of all, narrowing until it is gone.
5. A row still too wide once nothing more gives way is cut at its edge
   with `…`.

The columns before the first hugging one line up: they give way by the
widths measured over the whole list, so every row drawn in one room loses
the same ones. The hugging column and those after it give way row by row,
from each row's own cells. A row's last cell needs only its own text, not
its column's pad. A blank lined-up cell is padded while a later cell on its
row shows; a row ends at its last cell that is not blank.

## Key table

None: a schema lays rows out and handles no key. The list that draws the
rows (a tree, a palette) has the keys.

## Theme tokens

| Token | Used for |
| --- | --- |
| `muted` | `Cell::secondary` text |
| `recency_fresh`, `recency_active`, `recency_recent`, `recency_cold`, `muted` | the age cell, through `age::cell` |
| `disclosure` | `tree::disclosure` |

Every other style is the cell's own, from the host. Gaps and the pads of a
spans cell are drawn in the default style; a text cell's pad takes the
text's style, so a padded text cell is one run.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 13 | Theme | cells carry the host's styles; the schema adds only the tokens above |
| 14 | Width | every width counts terminal cells; a cut ends in `…` (or keeps both ends for a middle cut); a row never runs past its room without the mark |

Not applicable: 1 and 10 (the cursor's look is the list's: the tree
applies it after the producer), 2, 3 to 9, 11, 12, and 15 to 18 (no keys,
no overlay, no editing).

## Seams left open

- **The cursor gutter.** House rule 1's pointer marker in a two-cell
  gutter is not yet drawn before a tree's rows; a column of the schema
  is where it goes, once every kind of row a host draws is on one.
- **Shares of the row.** The palette's `ColumnPlan` has a width rule that
  takes a share of the row; a schema has none yet. A search result list
  that lays its hits out like the palette needs one, or the plan moves
  onto the schema.
- **Columns across depths.** `absorbs_lead` lines a column's end up at
  every depth; a host that wants its labels' columns to line up across
  depths sets it on the label column too.
- **Measured once per frame.** The measure pass asks a host for every
  row's cells each frame; a host with very many rows could keep the
  measure until its rows change.
- **Plain-text tables.** A table printed outside a terminal UI could lay
  its columns out with a schema; the allocation that shrinks the widest
  column first is not a width rule here.
