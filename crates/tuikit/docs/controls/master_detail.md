# `StackedMasterDetail`

| | |
| --- | --- |
| Module | `tuikit::master_detail` |
| Layer | 1 (component) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: `render` returns a `Stacked`, the list's window and the three areas |

## Purpose

A pane that lists the parts of one thing and shows the part under its
cursor below them stacks three zones in one column: a list sized to its
rows, a one-row divider, and a detail area. `StackedMasterDetail` sizes the
three and draws the first two: the list as tall as its rows wrap to,
within a minimum (three rows unless set) and the room a minimum detail
area (six rows unless set) leaves; the list drawn in a window that keeps
the cursor's whole line in view, with the shared scrollbar gutter; the
divider under it; and the detail area handed back for the host to draw in.

It replaces arithmetic a host writes inline in one pane's draw function,
which only a whole-frame test reaches: a floor for the detail, a fallback
for a short pane, a minimum for the list, a wrap that has to count the
rows it draws, and a layout width one column narrower than the pane so a
rule cannot wrap when the scrollbar appears. The host keeps what the zones
hold: the list's lines (and any block it puts above them, which moves the
cursor's line), the divider's words, and the detail.

## Usage example

A pane that lists the fields of one record over a preview of the field
under the cursor. The host lays its lines out at the layout width, names
the detail with a section rule at the pane's width, draws through the
layout with the window it kept from the last frame, keeps the window it
gets back, and draws the detail in the area the layout returns.

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use tuikit::frame::SectionRule;
use tuikit::master_detail::StackedMasterDetail;
use tuikit::property_grid::{GridLines, PropertyGrid, PropertyRow};
use tuikit::scroll::ScrollWindow;
use tuikit::theme::Theme;

let theme = Theme::default();
let area = Rect::new(0, 0, 40, 20);
let mut kept = ScrollWindow::default();
let cursor = 11;

// The list: rows of a property grid at the layout width.
let width = StackedMasterDetail::layout_width(area);
let mut grid = PropertyGrid::new();
for n in 0..24 {
    grid.push(PropertyRow::new(format!("field {n}"), format!("value {n}")));
}
let list: GridLines = grid.lines(width, Some(cursor), true, &theme);

// The divider at the pane's width, then the layout.
let divider = SectionRule::new("Preview").line(usize::from(area.width), &theme);
let mut buf = Buffer::empty(area);
let drawn = StackedMasterDetail::new().render(area, &mut buf, list, divider, kept);
kept = drawn.window;

// The list keeps the room six detail rows leave and scrolls to the cursor.
assert_eq!(drawn.list.height, 13);
assert_eq!(drawn.detail.height, 6);
assert!(kept.visible().contains(&cursor));

// The host draws the detail.
Paragraph::new(Line::from(format!("value {cursor}"))).render(drawn.detail, &mut buf);
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `StackedMasterDetail::new()` | the layout with the default minimums | `MIN_LIST` (3) rows of list, `MIN_DETAIL` (6) rows of detail; also `Default` |
| `.min_list(rows)` | the fewest rows the list takes | however few its rows and however short the pane |
| `.min_detail(rows)` | the rows the detail keeps however long the list | while the pane holds them beside the list's minimum and the divider |
| `StackedMasterDetail::layout_width(area)` | the width the host lays the list's lines out at | the area's less one column, at least 1 |
| `.areas(area, rows)` | the list's, the divider's, and the detail's areas for `rows` wrapped rows | draws nothing |
| `.render(area, buf, list, divider, window)` | wraps the list's lines, sizes the zones, draws the list in its window with the gutter and the divider under it | `list` is `GridLines` (lines and the cursor's line); `window` is the last frame's; returns `Stacked` |
| `Stacked { window, list, divider, detail }` | what was drawn | keep `window`; draw the detail in `detail` |

### The layout rule

For an area `height` rows tall:

1. The detail's floor is its minimum, or what the list's minimum and the
   divider leave when that is less: `min(min_detail, height − (min_list + 1))`.
2. The list's room is what the floor and the divider leave, but never less
   than its minimum: `max(height − (floor + 1), min_list)`.
3. The lines wrap at the area's width while they fit the room, and one
   column narrower when they do not (`WrappedRows::fit`), since the
   scrollbar gutter then takes that column.
4. The list takes its rows, at least its minimum and at most its room; the
   divider takes the row under it; the detail takes the rest.

With the defaults, a 30-row pane gives a long list 23 rows and the detail
6, a short list 3 rows and the detail 26; a 9-row pane gives a long list
3 rows and the detail 5. On an area too short for the list's minimum and
the divider (under four rows by default), the list keeps its first row,
the divider its row, and the list what is left; the detail gets none.

### Laying the list out

Lay the lines out at `layout_width`, one column narrower than the area. A
line as wide as its room (a section rule) then fits the content area
whether or not the scrollbar takes a column; laid out at the area's full
width it would wrap onto a second row when the bar appears, and every row
under it would land a row lower than the window counts, leaving the
cursor below the list. The layout itself wraps at the full width while
the rows fit, so a list that does not scroll loses no column.

### The window

`render` takes the window the last frame drew (`ScrollWindow::default()`
the first time), measures it again for the list's area and rows (the
offset comes back inside a list that shrank), and moves it as little as it
must to show every row the cursor's line took, so a wrapped cursor line is
shown whole; with no cursor line it shows the first row. It returns the
window as drawn. The host keeps it and pages the list by its `page`, the
rows the frame showed (see [scroll.md](scroll.md#the-window)).

## Key table

None: the layout draws and handles no key. The list's keys are its
control's (a property grid's cursor, an explorer's moves), which move the
cursor whose line the window follows.

## Theme tokens

None of its own. The scrollbar is the shared gutter's look
([scroll.md](scroll.md#the-gutter)); the list's lines and the divider
come styled from the host (a property grid's rows, a
[`SectionRule`](frame.md#sectionrule)).

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 10 | Unfocused cursor | The layout draws the lines as given, so a list laid out unfocused keeps its inactive cursor look; the window follows the cursor's line focused or not |
| 12 | Paging | The window is clamped on every frame, so no blank rows hang under the list while rows above it are scrolled away; the window returned is the one a page key pages by |
| 13 | Theme | No color is named; the gutter draws the one scrollbar look |
| 14 | Width | The rows are wrapped in terminal cells by `WrappedRows`, which counts the rows it draws, and the layout width keeps a rule from wrapping when the gutter takes its column |

Not applicable: 1 and 2 (the list's cursor and its ends are its
control's), 3 to 8, 11, and 15 to 18 (no keys, no overlay, no editing, no
pager), and 9 (the focus cue is the pane frame's, around the layout).

## Seams left open

- **The list's room.** The room the list may take is private: a host that
  wraps its own rows before calling `areas` counts them at the area's
  width. Exposing the room would let it wrap exactly as `render` does.
- **Side by side.** The split pane (planned in `frame`) puts two panes side
  by side above a width and stacks them below it; this layout is the
  stacked form of one pane and would be one of its children.
- **A divider of more rows.** The divider is one row. A taller one (a
  heading with a summary line) would be a height parameter, with the rule
  above unchanged.
- **Mouse hit testing.** `Stacked` reports the three areas and the window,
  which is what a click on a list row or the detail would be mapped
  through.
