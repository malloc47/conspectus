# `TabBar`

| | |
| --- | --- |
| Module | `tuikit::tab_bar` |
| Layer | 1 (component) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: `line` returns the bar's spans for the room it is given |

## Purpose

A pane that switches among a few views names them all where its reader
looks first, usually its title, so the views can be seen at a glance
instead of remembered: `one · two · three`, the active one in the accent
and bold, the rest receding. The bar sits beside other parts of a title, so
it gets whatever room they leave, and when that is not enough it gives way
tab by tab instead of letting the pane's border cut a name in half.
`TabBar` draws the bar in a room by the one fit rule of
[`text::elide`](text.md#giving-way): the inactive tabs give way from the
end of the bar, the active tab never, and a shortened bar ends with ` …`.

It replaces a host's hand-made strip with a fit routine of its own (or
none, so the border clipped the last names). The host keeps what the tabs
are: their names, their order, which is active, and the keys that switch
them.

## Usage example

A pane's title naming four views, the second active, in the room the pane
frame leaves.

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuikit::frame::PaneFrame;
use tuikit::tab_bar::TabBar;
use tuikit::testkit::buffer::rows;
use tuikit::theme::Theme;

let theme = Theme::default();
let views = ["inbox", "drafts", "sent", "archive"];
let bar = TabBar::new(&views, 1, &theme);

// A wide pane names every view.
let area = Rect::new(0, 0, 40, 3);
let mut buf = Buffer::empty(area);
PaneFrame::new(&theme)
    .title(bar.line(PaneFrame::title_width(area.width)))
    .focused(true)
    .render(area, &mut buf);
assert_eq!(rows(&buf)[0], "┌ ► inbox · drafts · sent · archive ───┐");

// A narrow one names fewer and says so; the active view always shows.
let area = Rect::new(0, 0, 24, 3);
let mut buf = Buffer::empty(area);
PaneFrame::new(&theme)
    .title(bar.line(PaneFrame::title_width(area.width)))
    .render(area, &mut buf);
assert_eq!(rows(&buf)[0], "┌   inbox · drafts … ──┐");
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `TabBar::new(&names, active, &theme)` | the tabs, in order, `active` the active one | an index past the end is the last tab |
| `.active_style(style)` | draw the active tab in `style` | default: `accent`, bold |
| `.style(style)` | draw the other tabs, the separators, and the mark in `style` | default: `inert` |
| `.active()` | the active tab's index, held inside the tabs | |
| `.fit(room)` | which tabs show, whether the mark ends the bar, and the cut | a `text::Elided` |
| `.line(room)` | the bar in at most `room` cells | an empty line with no tabs |

### The rule

The bar is one line of pieces for `text::elide`, with the mark at the end
(`MarkAt::End`): the separator is `marks::SPACED_SEPARATOR` (` · `, three
cells) and the mark `marks::MORE` (` …`, two cells), counted with one more
cell that stays blank after it. The active tab is a piece that never gives
way; every other tab has one rank, so of two the later gives way first.

- Every tab shows while they all fit, with nothing after the last.
- Otherwise inactive tabs give way from the end of the bar until the
  rest, the mark, and its blank cell fit. The blank cell keeps the mark off
  whatever follows the bar (a border's corner).
- When the active tab and the mark do not fit, the active tab shows alone
  with no mark, cut with `…` only when it alone is wider than the room.

So a bar narrows as `one · two · three · four`, `one · two · three …`,
`one · two …`, `one …`, `one`, and then `one` cut. A name is never cut
while another tab shows, the active tab always shows, and the line never
takes more cells than its room. Widths are terminal cells.

## Key table

None: the bar draws a line and handles no key. Switching views is the
host's keys (a number per view, a key that cycles), resolved in its own
key table.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | the active tab, bold, unless `.active_style` replaces it |
| `inert` | the other tabs, the separators, and the mark, unless `.style` replaces it |

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 13 | Theme | takes `&Theme` and draws in its `accent` and `inert` tokens or styles the host supplies; no color is named |
| 14 | Width | every width is measured in terminal cells through `text`; the bar gives way by whole names and ends in `marks::MORE`; the one cut ends in `marks::ELLIPSIS` inside the room |
| 17 | Labels | a tab shows the host's short name for its view |

Not applicable: 1, 2, and 10 (no list or cursor), 3 to 8, 11, 12, 15, 16,
and 18 (no keys, no overlay, no editing, no paging), and 9 (the focus cue
is the pane frame's, around the bar).

## Seams left open

- **Clicking a tab.** `fit` reports which tabs are kept and every name's
  width is known before drawing, which is what a mouse hit test on the
  title would map a click through to switch views.
- **Other orders.** The inactive tabs go from the end of the bar. Keeping
  the tabs beside the active one longest would be other ranks for the same
  rule.
- **Key hints on the tabs.** A tab could carry the key that switches to it
  (`1 inbox`); the name is the host's text, so it can today, at the cost of
  width.
