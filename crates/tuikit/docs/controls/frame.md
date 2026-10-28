# `ModalFrame`

| | |
| --- | --- |
| Module | `tuikit::frame` |
| Layer | 1 (foundation) |
| Type parameters | none; a body is any type that implements `ModalBody`, and its `Output` is what drawing it returns |
| Cargo feature | none |
| Outcomes | none: the frame handles no keys; drawing returns the body's output (the window a scrolled body showed, or `()`) |

## Purpose

`ModalFrame` is the box every modal draws in: a prompt, a checklist, a
menu, a settings list, a form, a reader, a long list, a search palette.
It has one look and one sizing rule:

- **Named sizes.** A `ModalSize` says how large the modal is on a screen
  of any size, from a few parameters per direction (an `Extent`: a share
  of the screen, a cap, a minimum, a margin) and, for the height, a `Fit`
  (as tall as the body, or as tall as the extent allows). Five named
  policies cover the usual modals; a host with another need builds its
  own from the same parameters rather than computing a rect.
- **One frame.** The rect is cleared and painted with the theme's
  `surface`, so nothing drawn under the modal shows through. The border
  is plain lines on all four sides in the `accent`. The title is plain
  text in the border's color and regular weight, left-aligned with one
  space on each side; any spaces a caller put at its ends are dropped.
- **Measured once.** A `ModalBody` is told the inner width its policy
  gives (`measure`), reports the rows it needs, and draws in the area the
  frame leaves (`render`). The frame places itself from those rows. No
  body reports a size it computed from a rect the frame computes again.

The module replaces what a program grows one modal at a time: a centered
rect function per modal with its own cap, minimum, and margin (some
leaving two cells clear, some flush with the screen's edges), frames
built by hand beside a shared helper, a popup crate whose body had to
echo back the size its caller had just computed, titles padded by the
caller or by the widget or not at all, and one or two titles in bold
where the rest are not. The host keeps what is in the modal: its rows,
its title text, its keys, and which named size suits it.

## Usage example

A menu laid out for the width its policy gives, drawn as a `Lines` body
that keeps the cursor's entry in view, and a reader whose body fills the
frame and reports the window it showed.

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use tuikit::frame::{Lines, ModalBody, ModalFrame, ModalSize};
use tuikit::scroll::ScrollWindow;
use tuikit::text;
use tuikit::theme::Theme;

let theme = Theme::default();
let area = Rect::new(0, 0, 80, 24);
let mut buf = Buffer::empty(area);

// A menu: lay the entries out for the frame's inner width, then draw.
let frame = ModalFrame::new(&theme, ModalSize::MENU).title("Actions");
let width = usize::from(frame.inner_width(area));
let entries = ["Open", "Rename", "Delete"];
let cursor = 1;
let lines: Vec<Line> = entries
    .iter()
    .enumerate()
    .map(|(i, entry)| {
        let marker = if i == cursor { "> " } else { "  " };
        Line::from(text::truncate_end(&format!("{marker}{entry}"), width))
    })
    .collect();
frame.render(area, &mut buf, Lines::new(lines).following(cursor..cursor + 1));

// A reader: it fills its frame, so it lays itself out when it draws.
struct Page<'a> {
    text: &'a str,
    offset: usize,
}

impl ModalBody for Page<'_> {
    type Output = ScrollWindow;

    fn measure(&mut self, _width: u16) -> u16 {
        0
    }

    fn render(self, area: Rect, buf: &mut Buffer) -> ScrollWindow {
        let rows = text::wrap(self.text, usize::from(area.width));
        let window = ScrollWindow::new(usize::from(area.height), rows.len()).at(self.offset);
        Paragraph::new(rows.join("\n"))
            .scroll(window.paragraph_scroll())
            .render(area, buf);
        window
    }
}

let page = Page { text: "a long page of help", offset: 0 };
let shown = ModalFrame::new(&theme, ModalSize::WIDE)
    .title("Help")
    .render(area, &mut buf, page);
assert_eq!(shown.viewport(), 20, "24 rows, less a row clear above and below and the border");
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `ModalFrame::new(&theme, size)` | a frame sized by a policy | no title, no padding |
| `title(line)`, `padding(padding)` | set the title and the padding | the title's spans keep their styles (chips); padding is ratatui's `Padding` |
| `inner_width(area)` | the columns a body gets | the policy's width less the border and padding |
| `rect(area, rows)` | the frame's rect for a body of `rows` rows | centered; a fill policy ignores `rows` |
| `render(area, buf, body)` | measures the body once, draws the frame, draws the body | returns the body's `Output` |
| `ModalBody::measure(width)`, `render(area, buf)` | the body's two calls | `measure` once per draw, before the frame is placed |
| `Lines::new(lines)`, `following(rows)` | a body of lines laid out ahead, scrolled to keep `rows` in view | one row per line; returns the window it showed |
| `ModalSize { width, height, fit }` | a policy | `width(screen)`, `height(screen, wanted)`, `rect(area, wanted)` |
| `Extent::new(cap, min, margin)`, `share(percent)` | one direction of a policy | `cells(screen, wanted)` applies the rule |
| `Fit::Content`, `Fit::Fill` | where the height comes from | the body's rows, or all the extent allows |

### Sizing policies

Each direction follows one rule: aim for the extent's share of the
screen, take no more than the cap and no more than the screen less the
margin on each side, then grow to the minimum, and never past the
screen. The margin gives way to the minimum on a small screen, and the
screen bounds everything, so a modal never leaves the screen. A body
that fits its content asks for its rows plus the border and padding; one
that fills asks for everything. The rect is centered, with an odd cell
left over going before it (ratatui's own centering).

| Policy | Width | Height | Margin | For |
| --- | --- | --- | --- | --- |
| `COMPACT` | up to 60, at least 20 | the body's, at least 3 | 2 columns, 1 row | a one-line prompt, a short checklist |
| `MENU` | up to 64, at least 40 | the body's, at least 3 | 2 columns, 1 row | a list of choices, a settings list, a short notice |
| `FORM` | up to 80, at least 44 | the body's, at least 3 | 2 columns, 1 row | labeled fields |
| `WIDE` | up to 100, at least 40 | all, at least 10 | 2 columns, 1 row | reading (help, a log, a long value); a long list that is filtered |
| `PALETTE` | 85% of the screen, 60 to 200 | 80%, 12 to 50 | 2 columns, 1 row | a search palette, which shows more of each result on a large screen |

Every policy keeps two columns and one row clear on each side while the
screen allows, so the screen's edges and its first and last lines (where
a host usually keeps a header and a status line) stay in view around a
modal. A body taller than its frame scrolls; a list that is filtered as
the user types uses a fill policy, so its frame keeps its size.

### Bodies

The frame calls `measure` once, with the inner width its policy gives,
before it places itself, and `render` once, with the inner area. A body
that fits its content lays itself out in `measure` (or is laid out ahead
of the draw for `inner_width`, as `Lines` is) and draws what it laid
out. A body drawn in a fill frame learns its height only in `render`, so
it lays itself out there and may report 0 rows. The area `render` gets is
as wide as `measure` was told and may be shorter than the body asked for
when the screen is short; the body scrolls. A body that scrolls returns
its window as its `Output`, which the host keeps for the next key and
frame (see [scroll.md](scroll.md#the-window)).

## Key table

None: the frame handles no keys. The modal inside it has its own key
table, and its footer is part of its body.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | the border, and so the title, which takes the border's color |
| `surface` | the background painted over the cleared rect |

A title's chips (a segmented choice) keep the look their own module gave
them, from `badge` and `accent`. The body styles itself.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 8 | Footers and hints | The frame draws no hint of its own; a modal's footer is its body's, generated from its key table |
| 13 | Theme | The border and background come from `accent` and `surface`; the frame names no color |
| 14 | Width | A body is laid out for the inner width the policy gives in cells, so nothing is laid out for one width and drawn at another |

Not applicable: 1 to 7, 9 to 12, and 15 to 18 govern how lists move, how
modals confirm, close, edit, and page, and how panes show focus. The
frame only places and draws the box; the modals inside it follow those
rules. Rule 9 belongs to the pane frame (below).

## Seams left open

- **The pane frame.** A bordered pane with the focus cue on its border
  and title joins this module, drawing the same border and title form
  through the same private helper, with the border in
  `panel_focus_accent` when focused.
- **Rules and split panes.** The horizontal rule and the split pane join
  this module with the pane frame.
- **Mouse hit testing.** `rect(area, rows)` is the rect the frame drew
  for the same rows, which a click on or outside the modal would be
  tested against.
- **Border looks.** The border is plain lines; a theme token naming a
  border set (rounded, thick, double) would let a host change every
  frame at once.
- **Title alignment.** Titles sit at the left; a centered or right title
  would be a parameter of the one title form, not a caller's choice of
  padding.
