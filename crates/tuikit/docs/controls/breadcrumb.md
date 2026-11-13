# `Breadcrumb`

| | |
| --- | --- |
| Module | `tuikit::breadcrumb` |
| Layer | 1 (component) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: `line` returns the chain's spans for the room it is given |

## Purpose

A pane that lets its reader follow a link from one thing to the next shows
how it got there: a chain of hops, each a glyph in the host's style and a
short tag, joined by `›` (`a › b › c`). The chain usually sits in a title
beside other parts, so it gets whatever room they leave, and when that is
not enough it has to give way without the pane's border cutting it off.
`Breadcrumb` draws the chain in a room by the one fit rule of
[`text::elide`](text.md#giving-way): the hops give way one at a time, the
oldest middle hops first, each run that gave way leaves one `…`, and the last
hop is cut only when it alone does not fit.

It replaces a host's hand-made ladder of fixed forms (every hop, then first
and last around a mark, then the last hop alone) under a floor that let the
border clip the title. The host keeps what a hop is: which glyph and color
stand for the kind of thing it was, which tag names it, which suffix tells
two alike apart, and every other part of the title (a focus marker, a
label, a "go back" mark, a depth count).

## Usage example

A title that shows where its pane is and the chain that led there, in the
room the rest of the title leaves.

```rust
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use tuikit::breadcrumb::{Breadcrumb, Hop};
use tuikit::glyphs::marks;
use tuikit::text;
use tuikit::theme::Theme;

let mut theme = Theme::default();
theme.register("kind.folder", Color::Blue, "a folder's glyph");
theme.register("kind.page", Color::Green, "a page's glyph");
let kind = |name: &str| Style::default().fg(theme.color(name).unwrap_or(Color::Reset));

// The host builds a hop per step: a glyph in its kind's color, a tag,
// and a suffix that tells two hops with one tag apart.
let hops = vec![
    Hop::new("docs").glyph(Span::styled("#", kind("kind.folder"))).suffix("a1f0"),
    Hop::new("guide").glyph(Span::styled("*", kind("kind.page"))).suffix("77c2"),
    Hop::new("docs").glyph(Span::styled("#", kind("kind.folder"))).suffix("e93b"),
];

// The rest of the title, then the chain in the cells it leaves.
let title_room = 50;
let mut spans = vec![
    Span::raw(" "),
    Span::styled("notes", Style::default().add_modifier(Modifier::BOLD)),
    Span::styled(format!(" {} ", marks::BACK), Style::default().fg(theme.inert)),
];
let depth = Span::styled(format!(" · depth {}", hops.len()), Style::default().fg(theme.inert));
let room = title_room - text::spans_width(&spans) - depth.width() - 1;
let chain = Breadcrumb::new(&hops, &theme).line(room).expect("hops");
spans.extend(chain.spans);
spans.push(depth);
spans.push(Span::raw(" "));
let title = Line::from(spans);
assert!(title.width() <= title_room);
assert_eq!(title.to_string(), " notes ◄ # docs·a1f0 › … › # docs·e93b · depth 3 ");
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `Hop::new(tag)` | a hop named `tag` | no glyph, no suffix |
| `.glyph(span)` | draw `span` before the tag, a space between | the span keeps its own style; its width is measured |
| `.suffix(text)` | show `tag·text` when another hop has the same tag | an empty suffix shows nothing |
| `.tag()` | the hop's tag | |
| `Breadcrumb::new(&hops, &theme)` | the chain, oldest hop first | tags, separators, and marks in `inert` |
| `.style(style)` | draw the tags, separators, and marks in `style` | glyphs keep their own |
| `.line(room)` | the chain in at most `room` cells | `None` with no hops; an empty line with no room |

### The rule

The chain is one line of pieces for `text::elide`, with the mark in the gap
(`MarkAt::Gap`): the separator is `marks::SPACED_CRUMB` (` › `, three
cells) and the mark `marks::ELLIPSIS` (one cell). Each hop is a piece as wide
as it draws (its glyph, a space, its tag, and the suffix when shown), ranked
so that:

- the last hop (where the reader came from) never gives way;
- the first hop (where the path began) outlasts every middle hop;
- the middle hops give way oldest first.

So five hops narrow as `a › b › c › d › e`, `a › … › c › d › e`,
`a › … › d › e`, `a › … › e`, `… › e`, `e`, and then `e` cut at its end with
`…`. One mark stands for each run of hops that gave way. When the last hop
is cut, the marks go first, and the cut's mark is in the chain's style even
when it falls in the glyph. The line never takes more cells than its room;
widths are terminal cells, so a wide tag or glyph is measured as it draws.

A suffix is shown only when two hops have the same tag, so two hops that
would read alike can be told apart; a hop whose tag is its own shows no
suffix.

## Key table

None: the breadcrumb draws a line and handles no key. Going back along the
chain is the host's key, in the pane that shows it.

## Theme tokens

| Token | Used for |
| --- | --- |
| `inert` | the tags, the separators, and the marks, unless `.style` replaces it |

A hop's glyph is drawn in the style the host gives its span (usually a
named style it registered for the kind of thing the hop was).

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 13 | Theme | takes `&Theme` and draws in its `inert` token or a style the host supplies; no color is named |
| 14 | Width | every width is measured in terminal cells through `text`; a cut ends in `marks::ELLIPSIS` inside the room; the chain never runs past its room for the border to clip |
| 17 | Labels | a hop shows its short tag, and a suffix only where two tags would read alike |

Not applicable: 1, 2, and 10 (no list or cursor), 3 to 8, 11, 12, 15, 16,
and 18 (no keys, no overlay, no editing, no paging), and 9 (the focus cue is
the title's, which the host draws around the chain).

## Seams left open

- **Clicking a hop.** `text::elide` reports which hops are kept and where the
  marks stand, and each hop's width is known before drawing, which is what a
  mouse hit test on the title would map a click through to jump back.
- **Other orders.** The ranks are fixed (last, then first, then the newest
  middle hops). A host that wants another order, such as keeping a hop its
  reader marked, would pass its own ranks; the rule underneath already
  takes any.
- **A wider mark.** The mark is the one-cell ellipsis. A count of what gave
  way (`… 3 …`) would be a mark of another width, which the rule already
  measures.
