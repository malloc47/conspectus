# `chip`

| | |
| --- | --- |
| Module | `tuikit::chip` |
| Layer | 1 (foundation) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: each chip is a plain value that returns its text, its style, and a span |

## Purpose

`chip` draws short labels set apart from the text around them, in five
forms, each with one rule:

- **`Chip`**, a filled pill of a fixed width: the label cut (with the
  ellipsis mark) or padded to the theme's `badge_width` in terminal cells,
  inside one cell of padding on each side, all in one style, so the
  padding carries the fill and every chip in a column has the same
  footprint. A column that holds chips is `chip::width(theme)` cells.
- **`ToggleChip`**, a switch and its state with the key that flips it,
  `name·state (key)`, the state padded to the widest it can show.
- **`SegmentedTitle`**, a choice among a few words in a title, the chosen
  one in brackets: `[ One ] Two`.
- **`PositionChip`**, where a reader is: `[ n/m ]`.
- **`StatusChip`**, a short status at its natural width in a `Tone`,
  optionally led by a mark or a busy spinner frame.

The module replaces what a program grows one surface at a time: a pill
helper that counts characters instead of cells, a list overlay that
hard-codes the pill's width and ignores the theme, a footer that pads its
switch states by hand, three copies of the position text, a title whose
chosen word hard-codes the chip look, and status text styled inline at
every site. The host keeps what each chip means: which label, which color
for which kind of thing, when a switch is on, and the words of a status.

## Usage example

A row with a filled chip for its kind, a footer of switches and a
position, a title with a choice, and a status line.

```rust
use std::time::Duration;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use tuikit::chip::{
    self, Chip, PositionChip, SegmentedTitle, StatusChip, Tone, ToggleChip,
};
use tuikit::glyphs::marks;
use tuikit::text;
use tuikit::theme::Theme;

let mut theme = Theme::default();
theme.register("kind.alpha", Color::Magenta, "the alpha kind's chip");

// A row: the chip is always `chip::width` cells, so the name after it
// starts in one column on every row.
let fill = theme.color("kind.alpha").unwrap_or(Color::Reset);
let row = Line::from(vec![
    Chip::new("alpha").color(fill).span(&theme),
    Span::raw(" first item"),
]);
assert_eq!(text::spans_width(&row.spans[..1]), chip::width(&theme));

// A chip in a style of the host's own keeps the width rule.
let quiet = Chip::new("none")
    .style(Style::default().add_modifier(theme.muted))
    .span(&theme);
assert_eq!(quiet.content, " none     ");

// A footer: switches keep their width as they flip.
let states = ["on", "off"];
let wrap = ToggleChip::new("wrap", "off", false).states(&states).key("w");
let footer = Line::from(vec![
    PositionChip::new(41, 300).span(),
    Span::raw(" · "),
    wrap.span(&theme),
]);
assert_eq!(footer.to_string(), "[ 42/300 ] · wrap·off (w)");

// A title with a choice.
let mut title = vec![Span::raw(" search  ")];
title.extend(SegmentedTitle::new(&[("Here", true), ("Everywhere", false)]).spans(&theme));
assert_eq!(Line::from(title).to_string(), " search  [ Here ] Everywhere");

// A status line: a busy chip, then a warning.
let status = Line::from(vec![
    StatusChip::busy("saving", Duration::from_millis(250)).span(&theme),
    Span::raw("  "),
    StatusChip::new("offline", Tone::Warning).span(&theme),
]);
assert!(status.to_string().starts_with(marks::SPINNER[2]));
assert_eq!(
    status.spans[0].style,
    Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
);
```

### The API

| Item | Draws | Notes |
| --- | --- | --- |
| `chip::label_width(theme)`, `chip::width(theme)` | nothing | the label's cells (`badge_width`, at least `MIN_BADGE_WIDTH`) and the chip's footprint (2 more) |
| `Chip::new(label)` | ` label    ` | the theme's `badge` look, left-aligned, `badge_width` cells of label |
| `.color(color)`, `.style(style)` | | the fill's color under `badge`, or a style of the caller's |
| `.label_width(cells)`, `.align(alignment)` | | override the theme's width; place a short label at the start, center, or end |
| `.text(theme)`, `.look(theme)`, `.span(theme)` | | the text, its style, and both as one span |
| `ToggleChip::new(name, state, on)` | `name·state` | `on` picks bold `success`, off is `Tone::Inert` |
| `.states(words)`, `.key(key)` | `name·state  (key)` | pad the state to the widest word; add the key |
| `.text()`, `.look(theme)`, `.span(theme)` | | |
| `SegmentedTitle::new(&[(word, chosen)])` | `[ One ] Two` | `.spans(theme)`; `chosen_look(theme)` is the chosen word's style |
| `PositionChip::new(offset, total)` | `[ n/m ]` | `n` is `offset + 1` held inside `1..=total`; bold; `.text()`, `.span()` |
| `StatusChip::new(label, tone)` | `label` | natural width |
| `.mark(mark)`, `.bold()`, `.pad(before, after)` | `▲ label  ` | a leading mark and a space; emphasis; blank cells in the chip's style |
| `StatusChip::busy(label, elapsed)` | `⠹ label` | the spinner frame for `elapsed`, bold accent |
| `spinner_frame(elapsed)` | `⠹` | one of `marks::SPINNER`, a frame every 120 ms |
| `Tone::style(theme)` | | the tone's style (table below) |

### The width rule

A filled chip is exactly `label_width + 2` cells wide, whatever its label:

- The label width is the theme's `badge_width` (8 by default), or the
  chip's own `label_width`; neither goes below `MIN_BADGE_WIDTH` (2, one
  character and the mark).
- A longer label is cut with `text::truncate_end`, ending in the ellipsis
  mark inside the width. A two-cell character that would straddle the cut
  goes whole, so the cut can be a cell short; the padding makes it up.
- A shorter label is padded with spaces, at the end unless the chip is
  centered or right-aligned.
- One space of padding goes before and after, and the whole text is one
  span, so a background or a reversed look fills the padding too.

A surface that lays chips in a column, or reserves a column for one, sizes
it with `chip::width(theme)`, so a theme that changes `badge_width` moves
every chip and every column together.

### Toggle chips

`name·state (key)`: the name, the separator mark (`marks::SEPARATOR`), the
state padded to the widest word in `states` so the chip keeps its width as
it flips, then the key in parentheses. The key travels with the label so
the reader learns the switch and how to flip it in one place. A state
wider than every listed word is shown whole; list every word the state can
take.

### Status chips

A status chip has the width of its text. Its tone picks the theme token;
`bold` adds emphasis, `mark` leads it with a mark and a space, and `pad`
adds blank cells before and after in the chip's own style, for a chip that
keeps its gap to its neighbors as part of itself. A busy chip's spinner is
derived from how long the work has run, so it needs no animation state;
the host redraws often enough to move it.

## Key table

None: the chips handle no keys. A toggle chip shows the key the host
binds; the binding lives in the host's key table.

## Theme tokens

| Token | Used for |
| --- | --- |
| `badge` | the filled chip's look (reversed and bold by default) and the chosen word of a segmented title |
| `badge_width` | a filled chip's label width, so every chip in a column lines up |
| `accent` | the chosen word of a segmented title; `Tone::Accent` and busy chips |
| `info` | `Tone::Info` |
| `success` | an on toggle chip; `Tone::Success` |
| `muted` | `Tone::Muted`; with `inert`, `Tone::Inert` and off toggle chips |
| `inert` | `Tone::Inert` and off toggle chips |
| `warning` | `Tone::Warning` |
| `error` | `Tone::Error` |

A filled chip's fill color is the caller's: a color the host registered as
a named style (looked up with `Theme::color`) or one it computed. A chip
in a style of the caller's reads no token but `badge_width`. The position
chip is bold and reads no token.

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 13 | Theme | Every chip styles from theme tokens or a color or style the caller supplies; no chip names a color |
| 14 | Width | A filled chip measures and cuts its label in cells through `text`, ending in `marks::ELLIPSIS`; the footprint is fixed, so columns after it line up |
| 17 | Labels | A toggle chip is `name·state (key)` everywhere, its state words the host's short forms |
| 18 | Pagers | `PositionChip` is the `[ n/m ]` chip a full-screen reader shows |

Not applicable: 1 to 12, 15, and 16 govern how lists and controls move,
confirm, close, edit, and give feedback. The chips only draw labels; the
controls that hold them follow those rules.

## Seams left open

- **A chip registry.** A host maps its kinds to fill colors itself (often
  through named styles); a later table of kind to chip look would let a
  control draw a kind's chip from the kind alone.
- **Clickable chips.** A toggle chip's text and width are known before it
  is drawn, which is what a mouse hit test on the footer would map a click
  through.
- **Severity.** `Tone` is the look; `feedback::Severity::tone` maps each
  severity onto it, so a status chip and a toast of one severity agree.
- **Dimmed chips.** A host that draws a gone row dims its chips by patching
  the span's style; a `dim` variant of the look would carry that here.
