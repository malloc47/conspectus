# `TreeView<R>` and `TreeState<Id>`

| | |
| --- | --- |
| Module | `tuikit::tree` |
| Layer | 1 (control) |
| Type parameters | `Id`, the host's identity for a row (`Ord + Clone`), which the selection and the expanded rows are kept by across rebuilds; `R`, the host's row type, which implements `TreeRow` (its identity, depth, and whether it has children) or is the plain `Row<Id, P>` carrying the host's payload `P`. The pane also takes the host's producer `F: Fn(&R, &RowCtx) -> Line<'static>` |
| Cargo feature | none |
| Outcomes | none: the state's moves return the cursor's new position among the showing rows (or `None` when no row shows), its folds whether they changed, and `TreeView::render` the window it drew |

## Purpose

A tree lists nested things (groups and their members, items and their
parts) as a flat list of rows in display order, each with a depth; a row's
children are the deeper rows that follow it. `TreeState<Id>` keeps what the
user did to it, apart from the rows: the selected row's identity and the
position it showed at, the expanded rows, and the window the last frame
drew. `TreeView` draws it in a bordered pane: a title on the border, an
optional header line, the showing rows in a window that follows the cursor
with the shared scrollbar, an optional footer banner, and a placeholder
when no row shows.

It replaces what a program grows around its first tree: selection, folds,
scroll offset, and a duplicate-row tiebreaker as loose fields on its state;
a retention block copied into every place the rows are rebuilt; an
"is the selected row expandable" scan per fold key; a renderer that sends
the size it measured back as a message mid-draw; page keys that move by
the terminal's height instead of the rows the tree shows; and lookups into
the program's data inside the function that lays a row out.

The host keeps its rows (it builds them, owns them, and rebuilds them as
its data changes), their identities, what each row says (the producer lays
its cells out at the width it is told), which keys move the tree, the
default folds a tree opens with, and what a row's ghost, pending, and mark
states are (it works them out before the frame, as `RowState`s, so laying a
row out looks nothing up).

## Usage example

Groups of items, a step and a page, a frame whose window the host keeps,
and a rebuild that keeps the cursor on its row or, when the row is gone,
where it was.

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use tuikit::glyphs::marks;
use tuikit::text;
use tuikit::theme::Theme;
use tuikit::tree::{Row, RowCtx, RowMark, RowState, TreeState, TreeView};

/// Groups and their items, in display order.
fn rows(items: &[&'static str]) -> Vec<Row<String, &'static str>> {
    let mut rows = vec![Row { id: "fruit".into(), depth: 0, expandable: true, payload: "fruit" }];
    for item in items {
        rows.push(Row { id: format!("fruit/{item}"), depth: 1, expandable: false, payload: *item });
    }
    rows.push(Row { id: "tools".into(), depth: 0, expandable: false, payload: "tools" });
    rows
}

/// The host lays a row out: an indent, a disclosure mark, a bookmark, the
/// label cut to the width it is given.
fn line(row: &Row<String, &'static str>, cx: &RowCtx<'_>) -> Line<'static> {
    let disclosure = match (row.expandable, cx.expanded) {
        (false, _) => "  ".to_string(),
        (true, true) => format!("{} ", marks::EXPANDED),
        (true, false) => format!("{} ", marks::COLLAPSED),
    };
    let mark = if cx.mark == RowMark::None { "  " } else { "* " };
    let label = text::truncate_end(row.payload, cx.width.saturating_sub(6));
    Line::from(vec![
        Span::raw("  ".repeat(usize::from(row.depth))),
        Span::raw(disclosure),
        Span::raw(mark),
        Span::styled(label, Style::default().add_modifier(Modifier::BOLD)),
    ])
}

let theme = Theme::default();
let mut tree = rows(&["apple", "pear", "plum"]);
let mut state = TreeState::new();
state.expand_id("fruit".to_string());
state.home(&tree);
state.step(&tree, 2);
assert_eq!(state.selected().map(String::as_str), Some("fruit/pear"));

// What the host knows about each row, worked out before the frame.
let mut states = vec![RowState::default(); tree.len()];
states[4] = RowState { ghost: true, ..RowState::default() };

// Draw, and keep the window the frame drew.
let area = Rect::new(0, 0, 24, 5);
let mut buf = Buffer::empty(area);
let window = TreeView::new(&tree, &state, line, &theme)
    .focused(true)
    .states(&states)
    .title(Line::from(" fruit "))
    .empty("Nothing here.")
    .render(area, &mut buf);
state.set_viewport(window.expect("rows show"));

// A page moves by the rows the frame showed and stops at the end.
state.page_down(&tree);
assert_eq!(state.selected().map(String::as_str), Some("tools"));
state.step(&tree, -2);

// A rebuild keeps the cursor on its row, or where it was when the row is gone.
let before = state.begin_rebuild(&tree);
tree = rows(&["apple", "plum"]);
state.finish_rebuild(before, &tree, None);
assert_eq!(state.selected().map(String::as_str), Some("fruit/plum"));
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `TreeRow` (`id`, `depth`, `expandable`), `Row<Id, P>` | a row: its identity, its depth (0 at the top), whether it has children | a host implements the trait on its own row or uses `Row` with its payload |
| `TreeState::new`, `Default` | nothing selected, nothing expanded, an empty window | a plain value: a host keeps one per tree and saves it whole |
| `selected`, `position`, `cursor(rows)` | the selected identity, the position it was placed at, the position of its showing copy | the copy nearest the position wins when an identity shows twice |
| `step(rows, delta)`, `page_down`, `page_up`, `home`, `end` | move the cursor through the showing rows, stopping at the ends | a page is the kept window's page (one row before the first frame); each returns the new position, or `None` (nothing selected) when no row shows |
| `select(rows, id)`, `deselect` | jump to an identity a row has, shown or not; select nothing | a jump forgets the position, so it lands on the first showing copy |
| `expand(rows)`, `collapse(rows)`, `toggle(rows)` | fold the selected row | a row without children is left alone; each says whether the folds changed |
| `reveal(rows, index)` | expand the ancestors of the row at `index` | for a jump to a row inside closed groups |
| `is_expanded`, `expanded`, `set_expanded`, `expand_id` | read and set the folds | a host opens a tree on its default folds with `set_expanded` |
| `visible(rows)`, `visible_indices(rows)` | the rows that show, in display order | a row is hidden while an ancestor with children is collapsed |
| `begin_rebuild(rows)`, `finish_rebuild(before, rows, hint)`, `rebuild(old, new, hint)` | keep the cursor across a rebuild of the rows | the rules below; the host may change the folds between the two halves |
| `window`, `set_viewport(window)`, `scroll_to_top`, `placed_window(rows, viewport)` | the window the last frame drew; keep a frame's window; scroll it to the top; the window a frame would draw | the window follows the cursor by the one minimal-motion rule (`scroll::follow_offset`) |
| `TreeView::new(rows, state, line, theme)` | a pane over the rows, each laid out by `line` | focused, with no title, header, footer, placeholder, or states until set |
| `focused`, `states`, `title`, `header`, `footer`, `empty` | the pane's focus and slots | below |
| `TreeView::render(area, buf)` | draw the pane | returns the window it drew, or `None` when no row shows |
| `RowCtx` (`selected`, `focused`, `expanded`, `ghost`, `pending`, `mark`, `width`, `theme`), `cursor()`, `apply_looks(spans)` | what the producer is told about a row | `apply_looks` gives spans the looks the tree gives every row |
| `RowState` (`ghost`, `pending`, `mark`), `RowMark` (`None`, `On`, `Off`) | what the host knows about a row, index for index with the rows | worked out before the frame |
| `RowColumns<R>` (`schema`, `cells`), `TreeColumns::new(columns, rows, state, states, theme)`, `line`, `line_then` | a producer that lays each row out from the column schema the host names for it, after the row's indent (`INDENT` cells a level), every row of the tree measured once | the cells do not depend on the row's width or cursor; see [columns.md](columns.md) |
| `disclosure(expandable, expanded, theme)` | the disclosure arrow as a column cell | blank for a row without children |

### Rebuilding

The host rebuilds its rows whenever its data or its projection of it
changes; the cursor is put back by the first rule that applies:

1. on the identity it was on, when a row with it shows (the copy nearest
   the position it showed at);
2. at the position it showed at, or on the last showing row when fewer
   show, when that identity is gone;
3. on the host's hint, when nothing was selected before and the hint shows
   (a first load can open on a row the host picks);
4. on the first showing row.

With no rows showing, nothing is selected.

### The pane

| Slot | Shown | Drawn |
| --- | --- | --- |
| title | always, when set | on the top border, as the host laid it out for the border's width (the pane's less two) |
| footer banner | when the pane has more than four rows inside its border | at its foot: the host's lines wrapped to the width, at most three rows, in the host's style |
| header line | when what is left has more than three rows | above the rows |
| rows | when any row shows | the showing rows in the window, one row each, with a 1-column scrollbar gutter carved from the right edge when they overflow |
| placeholder | when no row shows | the host's text wrapped to the width, in the `muted` look |

The producer is called for the rows the window shows only, at the width
left beside the gutter. The tree then applies, in order, the pending look
(`selection::pending`) to a row whose state says so, the ghost look
(`selection::ghost`) to a ghost row, and the cursor's highlight
(`selection::highlight`, active with focus and inactive without) to every
row with the selected identity. A host whose placeholder copy costs
something to work out sets it only when it has no rows: the first row
always shows.

## Key table

None yet: the tree defines no key table of its own. The host's keymap binds
its keys to the state's moves, and its help page lists them; the
conventional binding is:

| Key | Action | Notes |
| --- | --- | --- |
| `Down`, `j` | `step(rows, 1)` | stops at the last row |
| `Up`, `k` | `step(rows, -1)` | stops at the first row |
| `PgDn`, `PgUp` | `page_down`, `page_up` | by the rows the last frame showed |
| `Home`, `g`, `End`, `G` | `home`, `end` | |
| `Right`, `l` | `expand` | a row without children is left alone |
| `Left`, `h` | `collapse` | the cursor stays on the row |
| `Enter` on a row with children | `toggle` | what `Enter` does on other rows is the host's |

## Theme tokens

| Token | Used for |
| --- | --- |
| `selection_active` | the cursor row while the tree has focus |
| `selection_inactive` | the cursor row while it does not |
| `inert` | the ghost look's color |
| `muted` | the pending look, a ghost row's chips, and the placeholder |

The border and the scrollbar use ratatui's default look; the footer banner
takes the style its host passes (a host draws advice in `warning`). Every
color in a row is the producer's.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | the cursor row takes the theme's selection pair through `selection::highlight`, a chip keeping its fill; the pointer gutter waits for the column schema (seams) |
| 2 | List ends | the cursor stops at the first and the last showing row |
| 10 | Unfocused cursor | an unfocused tree highlights its cursor row in `selection_inactive` |
| 12 | Paging | a page is the rows the last frame showed; `home` and `end`; the window is clamped inside the showing rows on every frame, so collapsing near the end leaves no blank rows below the content |
| 13 | Theme | the looks come from the theme; the control names no color |
| 14 | Width | the producer is told the cells it may take, the gutter's column already taken off |

Not applicable: 3 to 9, 11, and 15 to 18 govern confirmation, closing,
forms, errors, footers, the pane border's focus cue, feedback, editing,
paths, labels, and pagers. The tree handles no key itself and its border is
plain; the host's title carries its focus marker until the pane frame lands.

## Seams left open

- **The cursor gutter.** House rule 1's pointer marker in a 2-cell gutter
  is not drawn yet: a tree's rows start with columns (disclosure, kind
  glyph) that the host documents, and the gutter moves them. It comes as a
  column of the schema once every kind of row a host draws is on one;
  `selection::row` draws it.
- **Rows on a schema.** `RowColumns` and `TreeColumns` lay rows out from
  column schemas ([columns.md](columns.md)): one measure pass over the
  whole tree and one narrow-width policy. A host moves one kind of row at
  a time and keeps a producer of its own, or `TreeColumns::line_then`,
  for the rest.
- **A key table.** A `TREE_KEYS` table over a tree action enum would let a
  host resolve the keys above and generate its help from them, with the
  program-wide keymap.
- **Rows loaded later.** A row may say it has children before they are
  loaded (it is `expandable` with no deeper rows yet); the host loads them
  when it is expanded. A loading look on such a row is the host's today.
- **One highlighted copy.** Every copy of the selected identity is
  highlighted, and the window follows the copy the cursor is on. A
  highlight on that copy alone would need the pane to say which copy is the
  cursor's in `RowCtx`.
- **Mouse hit testing.** A click maps to a row through the window's visible
  range and the showing indices; the state needs nothing more.
- **Expand all, collapse all, jump to parent.** Each is a fold over the
  showing rows or a walk to an ancestor (`reveal` already walks them);
  none is built.
