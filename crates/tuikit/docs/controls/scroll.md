# `scroll`

| | |
| --- | --- |
| Module | `tuikit::scroll` |
| Layer | 1 (foundation) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: the steps return the moved index, the window, gutter, and region are plain values, the gutter's draw returns nothing, and a region's draw returns the region it drew |

## Purpose

`scroll` is how every scrolled surface moves, stops, and shows where it
is. A cursor steps through rows ([`wrap_step`] for short choice lists,
[`clamp_step`] for scrollable content), a window follows it by the
minimal-motion rule ([`follow_offset`], [`ScrollWindow`]), the window
clamps so it never scrolls past its content, a 1-column gutter appears
for the scrollbar only when the rows overflow ([`ScrollGutter`]), and a
body that wraps counts its rows from the rows it draws ([`WrappedRows`]).
A body with no cursor whose place counts from its end or from a row it
marks, rather than from its top, scrolls in a [`ScrollRegion`].

The module replaces what a program grows one surface at a time: a
scroll-to-cursor formula per list, a follow function per pane, a
scrollbar recipe copied into the one surface that could not reach the
shared helper, row estimators that disagree with the wrap, and offsets
that are never clamped, so a list that shrank shows blank rows below its
end while rows above are scrolled away. The host keeps what its rows
are, where its cursor is, which keys page, and where the window is kept
between frames; for a region, what it counts from.

[`wrap_step`]: #the-api
[`clamp_step`]: #the-api
[`follow_offset`]: #the-follow-rule
[`ScrollWindow`]: #the-window
[`ScrollGutter`]: #the-gutter
[`WrappedRows`]: #wrapped-rows
[`ScrollRegion`]: #the-region

## Usage example

A list that keeps its window between frames. The host stores the
window; each frame measures the area, follows the cursor, draws, and
hands the window back as a value for the host to keep.

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use tuikit::scroll::{ScrollGutter, ScrollWindow, WrappedRows, clamp_step};

struct List {
    items: Vec<String>,
    cursor: usize,
    /// What the last frame showed: kept for the next key and frame.
    window: ScrollWindow,
}

impl List {
    /// A page key moves by the rows the last frame showed.
    fn page_down(&mut self) {
        let page = i32::try_from(self.window.page()).unwrap_or(i32::MAX);
        self.cursor = clamp_step(self.cursor, self.items.len(), page);
    }

    /// Draw from a shared borrow and report the window drawn.
    fn draw(&self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let lines: Vec<Line> = self.items.iter().map(|item| Line::from(item.as_str())).collect();
        let rows = WrappedRows::fit(&lines, usize::from(area.width), usize::from(area.height));
        let gutter = ScrollGutter::layout(area, rows.len());
        let mut window = self.window;
        window.resize(usize::from(area.height), rows.len());
        window.follow(rows.rows_of(self.cursor));
        let scroll = window.paragraph_scroll();
        Paragraph::new(rows.into_rows())
            .scroll(scroll)
            .render(gutter.content(), buf);
        gutter.render(&window, buf);
        window
    }
}

let mut list = List {
    items: (1..=40).map(|n| format!("entry {n}")).collect(),
    cursor: 0,
    window: ScrollWindow::default(),
};
let area = Rect::new(0, 0, 24, 10);
let mut buf = Buffer::empty(area);
list.window = list.draw(area, &mut buf);
list.page_down();
list.window = list.draw(area, &mut buf);
assert_eq!(list.cursor, 10);
assert_eq!(list.window.visible(), 1..11);
```

A list that keeps no window builds one around its cursor each frame
(the same as following from offset 0), and a pager without a cursor
scrolls the window itself:

```rust
use tuikit::scroll::{ScrollWindow, follow_offset};

// The cursor on the last line once it passes the first screen.
assert_eq!(follow_offset(0, 14..15, 6, 30), 9);
let window = ScrollWindow::new(6, 30).following(14..15);
assert_eq!(window.visible(), 9..15);

let mut pager = ScrollWindow::new(6, 30);
pager.page_down();
pager.scroll_by(100);
assert_eq!(pager.visible(), 24..30);
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `wrap_step(index, len, delta)` | moves a cursor, wrapping past either end | short choice lists (house rule 2); 0 for an empty list |
| `clamp_step(index, len, delta)` | moves a cursor, stopping at the ends | scrollable content (house rule 2); a page passes the window's `page()` |
| `follow_offset(offset, rows, viewport, content)` | the offset that shows `rows`, moved as little as it must | the rule below; a list with no stored window passes 0 |
| `ScrollWindow::new(viewport, content)`, `at(offset)` | a window at the top, or at an offset | `at` clamps |
| `following(rows)` | a new window moved to show `rows` | for a list that keeps no window: `new(viewport, content).following(cursor..cursor + 1)` |
| `resize`, `follow` | measure again; show the cursor's rows | every frame, in that order |
| `scroll_by`, `page_down`, `page_up`, `to_top`, `to_bottom` | scroll a window with no cursor | each clamps |
| `offset`, `viewport`, `content`, `last_offset`, `visible`, `overflows`, `page` | read the window | `page` is at least 1 |
| `paragraph_scroll()` | the offset as `Paragraph::scroll` takes it | saturates at `u16::MAX` |
| `ScrollGutter::layout(area, rows)` | the content area and, on overflow, the gutter | fade on fit; no gutter below 2 columns |
| `ScrollGutter::render(&window, buf)` | draws the bar | nothing when there is no gutter or the window fits |
| `WrappedRows::new(lines, width)`, `fit(lines, width, height)` | lines wrapped by `text::wrap_line`, with each line's rows | `fit` narrows by the gutter's column on overflow |
| `WrappedRows::hanging(lines, width, indent)`, `cut(lines, width)` | lines wrapped under a hanging indent (a gutter such as line numbers), or one row each cut with `…` | `rows_of(line)` and `line_of(row)` map lines and rows both ways, for a pager's content |
| `len`, `lines`, `rows_of(line)`, `rows`, `into_rows` | read the rows | `rows_of` is the span to follow |
| `ScrollRegion::new(anchor)`, `set_anchor`, `marked(row)` | a region counting from `Anchor::Top`, `Bottom`, or `Mark`; the row a mark counts from | another anchor starts the region at it; the same anchor keeps its place |
| `scroll_by(delta)`, `reset` | step the region, down when positive; go back to the anchor | a step toward the anchor's own end stops there at once |
| `place(anchor, viewport, content)` | the region placed over the rows a frame laid out | clamps; the host keeps the result |
| `draw_lines(anchor, area, buf, text)`, `draw_cut(area, buf, text)` | draw lines the region wraps and windows, or rows the host cut at the placed window under a banner | both draw the scrollbar and return the region drawn |
| `moved`, `away`, `mark`, `window`, `anchor` | read the region | `away` is the distance from the anchor either way |
| `rows_under(banner, width, height)` | the rows a body gets under a banner | at least one |

### The follow rule

`follow_offset` moves the window only as far as it must ("minimal
motion"):

- The offset first comes back inside the content: never past
  `content - viewport`, and 0 when the content fits.
- Rows that end below the window bring their last row to the window's
  last line; rows that start above it bring their first row to its first
  line. A span taller than the window shows its start, the row that names
  the item.
- A cursor that moves inside the window does not scroll it. Nothing
  centers the cursor and there is no scroll margin.
- A window of no rows shows nothing, so only the clamp applies.

### The window

A `ScrollWindow` is `Copy` and holds an offset, the rows it shows at
once (the viewport), and the rows there are (the content). Every
constructor and step clamps, so after any sequence of steps, pages,
jumps to the ends, and resizes the offset is at most `content -
viewport`; a property test drives random sequences through a cursor and
a window and checks that the cursor stays inside the window and the
window inside the content. The window holds no cursor: a list keeps its
own and passes its rows to `follow`.

The host keeps the window. A frame resizes it to the area it measured,
follows the cursor, draws with its offset, and returns it, and the host
stores it after the frame, so the next page key moves by the rows the
user saw. Drawing needs only a shared borrow of the host's state: no
message is sent from inside a draw and no state is shared behind
interior mutability.

### The gutter

`ScrollGutter::layout` gives a body all of its area when its rows fit,
and otherwise the area less its right column, which becomes the
scrollbar's gutter. The bar is never drawn over the body, so a
highlighted row cannot run under it. `render` draws ratatui's vertical
scrollbar in the gutter with a 1-row margin at the top and the bottom,
so the track keeps clear of a border or a rule above and below. The
thumb reaches the bottom of the track exactly when the window is at its
last offset, its length is the window's share of the content, and the
bar removes the reversed modifier from its cells so a selection
highlight painted there earlier does not show through.

### Wrapped rows

A body that wraps must know its row count before it draws, for the
window, the scrollbar's length, a height budget, and the span a wrapped
cursor line takes. `WrappedRows` wraps with `text::wrap_line` and keeps
the rows, so the surface draws exactly what it counted (a paragraph
without its own wrap). `fit` wraps at the full width when the rows fit
and one cell narrower when they overflow, which is the width
`ScrollGutter::layout` then leaves. For one-cell text with words one
space apart the rows match what ratatui's paragraph wrap draws; where the
text module's [wrapping rules](text.md#wrapping-rules) differ on purpose
(no continuation row starts with a space, a wide character never runs
past the edge), the rows follow the text module.

### The region

A [`ScrollWindow`] counts its offset from the first row. A body that
shows the newest rows of something (a log, a terminal's output) or opens
on a match wants its place kept relative to that instead: scrolled up
three rows from the end, it should stay three rows from the end when the
next frame lays out more rows. A `ScrollRegion` is that window. It holds
an `Anchor`, the rows it has moved from it, the row a mark counts from,
and the window its last placement showed, and like the window it is a
`Copy` value the host keeps.

| Anchor | Opens at | Steps |
| --- | --- | --- |
| `Top` (the default) | the first row | down moves away; up stops at the first row at once |
| `Bottom` | the last page: the newest rows | up moves away; down stops at the last row at once, and the region keeps its distance from the end as rows arrive |
| `Mark` | the marked row at the top (the host passes a match's first row less its context) | either way from the mark; a mark on the last page opens on the last page |

A step past an end the anchor does not stop at (above the first row
counting from the bottom, below the last counting from the top, either
end counting from a mark) is taken back by the next placement, so the
next step the other way moves at once. A
different anchor (a match shown, then gone, on the same body) starts the
region at it rather than carrying an offset counted from somewhere else.

Each frame the host places the region over the rows it laid out and
draws it one of two ways:

- **Lines** (`draw_lines`): the host hands every line; the region wraps
  them to the area (a cell narrower beside the scrollbar when they
  overflow, by `WrappedRows::fit`), places itself over the rows, and draws
  its window. A banner the host puts first is the lines' start: it
  scrolls away with them. A `Mark` is a line, counted from its first row.
- **Cut** (`draw_cut`): the host lays out a long body itself (keeping the
  layout between frames), calls `place` for the window, cuts and paints
  only those rows, and hands a banner, if any, then the rows. The banner
  stays put above them, the rows the window counts are the body's alone,
  and the scrollbar tracks the window. `rows_under` is the height the
  body is placed in under the banner.

Either way the draw returns the region as drawn, which the host stores
after the frame, so the next key steps from what the person saw. A body
still waiting for its content draws a placeholder and keeps the region
it had, so keys pressed while it loads are not lost.

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Text};
use tuikit::scroll::{Anchor, ScrollRegion};

// A body that shows the newest of 40 lines, scrolled up two.
let mut region = ScrollRegion::new(Anchor::Bottom);
region.scroll_by(-2);
let text = Text::from((1..=40).map(|n| Line::from(format!("line {n}"))).collect::<Vec<_>>());
let area = Rect::new(0, 0, 20, 5);
let mut buf = Buffer::empty(area);
let drawn = region.draw_lines(Anchor::Bottom, area, &mut buf, text);
assert_eq!(drawn.window().visible(), 33..38);

// The host keeps `drawn`; a step down past the end stops at the end.
let mut kept = drawn;
kept.scroll_by(5);
assert_eq!(kept.moved(), 0);
```

## Key table

None: the module handles no keys. A list or pager maps its own keys to
these steps: `Up`/`Down` to a step of 1, `PgUp`/`PgDn` to the window's
`page()`, `Home`/`End` to the first and last row (house rule 12). A
region's host routes the keys that scroll it to `scroll_by`; a region
beside a list whose `j`/`k` move the cursor takes keys of its own, such as
`J`/`K`.

## Theme tokens

None: the scrollbar draws ratatui's default track and thumb, and only
removes the reversed modifier from its cells. Thumb and track tokens are
a seam below.

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 2 | List ends | `wrap_step` for short choice lists, `clamp_step` for scrollable content |
| 12 | Paging | `page()` is the rows the last frame showed, so a page key moves by the viewport; every window clamps, so no offset passes the end, and a region's offset is brought inside its rows by every placement, whatever it counts from |
| 14 | Width | `WrappedRows` wraps through the text module, measuring cells |

Not applicable: 1, 3 to 11, 13, and 15 to 18 govern how controls look,
confirm, close, label, and edit. The module only steps, follows, and
measures; the controls that use it follow those rules. Rule 13 is met
trivially: the module names no color.

## Seams left open

- **Scrollbar tokens.** The bar uses ratatui's default look. Thumb and
  track roles in the theme would style it; `render` would take `&Theme`.
- **Half pages.** House rule 12's half-page `Ctrl-D`/`Ctrl-U` in pagers is
  a step of half the window's `page()`; a pager adds the keys when it
  lands.
- **The end that sticks.** A region counting from the bottom keeps its
  distance from the end, and the pager keeps its window at the end once
  `G` sticks it; a plain `ScrollWindow` has neither, and a flag that
  `resize` honors would carry it.
- **Region keys.** A region steps by rows only; paging it by its window
  and jumping to its ends would be `page()` steps and `reset` on keys its
  host binds.
- **Scroll margins.** A margin (keep N rows above and below the cursor)
  would be a parameter of the follow rule; today there is none.
- **Mouse hit testing.** `ScrollGutter::bar` and the window's `visible`
  range are what a click on the track or a row would be mapped through.
- **Virtual rows.** A very long list could draw only the visible range
  instead of handing every row to the paragraph; `visible` names it.
