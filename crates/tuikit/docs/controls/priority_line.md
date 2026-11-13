# `PriorityLine`

| | |
| --- | --- |
| Module | `tuikit::priority_line` |
| Layer | 1 (component) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: `line` returns the line's spans for the room it is given |

## Purpose

A summary line or a status line shows several short things at once:
counts, settings, key hints, status chips. They are laid out in an order
that reads well, not in order of importance, so when the screen is narrow
the least important have to go first wherever they stand, and the line has
to say that something went instead of being cut mid-word at the edge.
`PriorityLine` holds the line's segments, each with a rank (its priority),
and draws what fits by the one fit rule of
[`text::elide`](text.md#giving-way), filled back by
[`text::elide_filled`](text.md#filling-the-room-back): segments give way
lowest rank first, a narrower segment comes back where a wider one of
higher rank did not fit, and the line ends with ` …` when something went.

It replaces a hand-written packing loop per line (a status bar that fits
its hints by priority, a header that has no fit at all and is cut by the
edge). The host keeps what the segments say and how important each is:
the words, the chips' meanings, the ranks, and the parts of the line that
never move, which it draws around the line.

A `StatusChip` from [`chip`](chip.md#status-chips) is a segment like any
other: its span, with whatever the host puts beside it.

## Usage example

A summary line: a freshness note, a count that always shows, a smaller
count, and a warning chip set apart by a wider gap.

```rust
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use tuikit::chip::{StatusChip, Tone};
use tuikit::glyphs::marks;
use tuikit::priority_line::{PriorityLine, Segment};
use tuikit::theme::Theme;

let theme = Theme::default();
let bold = Style::default().add_modifier(Modifier::BOLD);
let summary = PriorityLine::new(&theme)
    .separator(Span::styled(marks::SPACED_SEPARATOR, bold))
    .segment(Segment::new(Span::styled("synced 2m ago", bold)).rank(3))
    // No rank: the count never gives way.
    .segment(Segment::new(Span::styled("12 items", bold)))
    .segment(Segment::new(Span::styled("3 shared", bold)).rank(2))
    .segment(
        Segment::new(vec![
            StatusChip::new(marks::WARNING, Tone::Warning).bold().span(&theme),
            Span::raw(" 2"),
        ])
        .rank(1)
        .gap("  ·  "),
    );
let text = |room| summary.line(room).to_string();
assert_eq!(text(80), "synced 2m ago · 12 items · 3 shared  ·  ▲ 2");
assert_eq!(text(40), "synced 2m ago · 12 items · 3 shared …");
assert_eq!(text(30), "synced 2m ago · 12 items …");
assert_eq!(text(20), "12 items  ·  ▲ 2 …");
assert_eq!(text(5), "12 i…");

// A status line: a lead the host keeps whatever the width, then hints
// in the cells it leaves.
let lead = Span::raw("[list] ");
let hints = PriorityLine::new(&theme)
    .segment(Segment::new("j/k move").rank(3))
    .segment(Segment::new("Enter open").rank(2))
    .segment(Segment::new("q quit").rank(4));
let room = 32 - lead.width();
let mut status = vec![lead];
status.extend(hints.spans(room));
let status = ratatui::text::Line::from(status);
assert_eq!(status.to_string(), "[list] j/k move · q quit …");
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `Segment::new(spans)` | a segment drawing `spans` | a span, a string, or a list of spans; it never gives way until ranked |
| `.rank(rank)` | give way at `rank` | the lower, the sooner; of two with one rank, the later first |
| `.gap(span)` | draw `span` before the segment in place of the separator | not drawn before the line's first item |
| `.rank_of()`, `.width()` | the segment's rank and cells | |
| `PriorityLine::new(&theme)` | an empty line | separator `marks::SPACED_SEPARATOR` and mark `marks::MORE`, both `muted` |
| `.separator(span)`, `.mark(span)` | another separator or mark | the mark carries its own spacing |
| `.segment(segment)`, `.push(segment)` | add a segment after the others | |
| `.segments()`, `.is_empty()` | the segments | |
| `.fit(room)` | which segments show, whether the mark ends the line, and the cut | a `text::Elided` |
| `.spans(room)`, `.line(room)` | the line in at most `room` cells | an empty line with no segments |

### The rule

The line is one line of pieces for `text::elide_filled`, with the mark at
the end (`MarkAt::End`): each segment a piece as wide as its spans, with
its rank and the width of its gap.

1. Every segment shows while they all fit.
2. Otherwise segments give way lowest rank first, the later of a tie
   first, until the rest and the mark fit.
3. Then each segment that gave way comes back, highest rank first, when
   the line still fits with it. Segments of one rank come back in order,
   the earlier first, and once one does not fit the later ones of that
   rank stay out, so the segments kept of each rank are always its first
   ones. With every rank different this is packing by priority: in order
   of importance, keep each segment that fits beside the ones kept.
4. Segments with no rank never give way. When they alone do not fit, the
   mark goes and the last of them is cut with `…` to the cells the others
   leave.

A segment's gap is drawn before it whenever an item precedes it, so a
group of segments can sit apart from the rest (`a · b  ·  [x] 1  [y] 2`:
the first of the group with a wide gap, the rest with a narrow one; give
the group one rank so its segments go from its end and its first stays
longest). The line never takes more cells than its room, unless several
segments have no rank and those before the last already fill it. Widths
are terminal cells.

## Key table

None: the line draws and handles no key. A line of key hints names keys
the host's tables bind.

## Theme tokens

| Token | Used for |
| --- | --- |
| `muted` | the separator and the mark, unless `.separator` or `.mark` replaces them |

A segment's spans keep the styles the host gave them (often a token, a
named style, or a `StatusChip`'s tone).

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 13 | Theme | takes `&Theme` and draws its separator and mark in `muted`; the segments carry the host's styles; no color is named |
| 14 | Width | every width is measured in terminal cells through `text`; segments give way whole and the line ends in `marks::MORE`; the one cut ends in `marks::ELLIPSIS` inside the room |

Not applicable: 1, 2, and 10 (no list or cursor), 3 to 7, 12, 15, 16, and
18 (no keys of its own, no overlay, no editing, no paging), 9 (no pane),
11 (the segments carry their severity's style; the line only places them),
17 (the words are the host's), and 8 (a line of hints generated from key
tables is [`StatusBar`](status_bar.md), built on this line).

## Seams left open

- **Hints from key tables.** Built: [`StatusBar`](status_bar.md) puts a
  `KeyHintBar` of hints generated from key tables on this line, each a
  segment with its row's rank.
- **Clicking a segment.** `fit` reports which segments are kept and each
  segment's width is known before drawing, which is what a mouse hit test
  would map a click through.
- **Strict giving way.** The line always fills the room back. A line whose
  segments read in sequence uses `text::elide` directly (as `TabBar` and
  `Breadcrumb` do); a switch here would offer the same.
- **Right-aligned segments.** Segments that hug the right edge would be a
  second line laid out in the cells this one leaves.
