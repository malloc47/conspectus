# `ModalFrame`, `ModalFooter`, `PaneFrame`, `SplitPane`, and `SectionRule`

| | |
| --- | --- |
| Module | `tuikit::frame` |
| Layer | 1 (foundation) |
| Type parameters | none; a body is any type that implements `ModalBody`, and its `Output` is what drawing it returns |
| Cargo feature | none |
| Outcomes | none: the frames handle no keys; drawing a modal frame returns the body's output (the window a scrolled body showed, or `()`), drawing a pane frame returns the area inside its border; a split returns two areas; a section rule is a line |

## Purpose

`ModalFrame` is the box every modal draws in: a prompt, a checklist, a
menu, a settings list, a form, a reader, a long list, a palette.
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
  space on each side; any spaces a caller put at its ends are dropped. A
  footer, when a modal gives one, sits on the bottom border in the same
  form.
- **Measured once.** A `ModalBody` is told the inner width its policy
  gives (`measure`), reports the rows it needs, and draws in the area the
  frame leaves (`render`). The frame places itself from those rows. No
  body reports a size it computed from a rect the frame computes again.
- **One footer.** `ModalFooter` draws every modal's keys: in one look
  (the theme's `muted`, as the status bar draws hints), on the frame's
  last rows under a blank row, the keys that leave last, and fitted to the
  frame by one rule when they do not fit: the blank row goes first, then
  the keys keep their last rows and their first rows scroll after the
  body's last row (see [The footer](#the-footer)).

The module replaces what a program grows one modal at a time: a centered
rect function per modal with its own cap, minimum, and margin (some
leaving two cells clear, some flush with the screen's edges), frames
built by hand beside a shared helper, a popup crate whose body had to
echo back the size its caller had just computed, titles padded by the
caller or by the widget or not at all, and one or two titles in bold
where the rest are not; and footers drawn plain in some modals and dimmed
in others, under a blank row or not, that lost different keys on a short
screen. The host keeps what is in the modal: its rows,
its title text, its keys, and which named size suits it.

`PaneFrame` is the box a pane of the screen draws in: a list, a detail
view, anything that sits beside another pane rather than floating over
the screen. It shares the modal's title form and adds the focus cue
(house rule 9):

- **Border.** Plain lines on all four sides, in `panel_focus_accent`
  while the pane has focus and in the terminal's own look while it does
  not. A modal's border is always the accent; a pane's only while it is
  the one with focus, so the focused pane is the one framed in color.
- **Lead and marker.** The title starts with a space, the focus marker
  (`marks::POINTER` in `panel_focus_accent` while focused, a blank cell
  otherwise), and a space: `LEAD`, three cells either way, so the title
  never moves as focus does.
- **Title.** The host's title after the lead, in its own looks: the
  border's color does not reach it, so a title that styles a word (a kind
  glyph, an active tab) keeps that style and the rest stays plain. Spaces
  at its ends are dropped, as in a modal title, and the frame closes it
  with one space when the border has a cell left. A host lays its title
  out for `PaneFrame::title_width(width)`, the border between its corners
  less the lead.

`SplitPane` places two panes in one area by one rule: side by side in
halves while the area is at least its threshold wide, the first above the
second below it. An odd width (or an odd height when they stack) gives
its odd cell to the first pane. A host keeps one split and asks it for
the two areas wherever it needs them, to draw the panes and to find one
pane's cells again in a dev tool or a check, so the screen is never split
by a copy of the rule.

The pane frame replaces a bordered block each pane built for itself with
the focus marker spliced into its title by hand and a border that never
changed, and the split replaces the same fifty-fifty arithmetic kept in
the draw, the dev tool that prints a pane, and the checks that read one.
The host keeps the title's words and which pane has focus.

Inside a pane, a `SectionRule` heads each section: a rule that fills the
width in the theme's `muted`, the section's label as a chip in `accent`
under `badge`, an optional summary in `inert` before or after the chip,
and a two-cell closing rule. Every section rule puts its chip two cells
from the right end, so the labels of the sections down a pane line up;
with the summary first, the chip keeps that column however long the
summary grows. It replaces a divider function a host keeps beside one
pane, with an anchor enum whose names and docs disagreed about where the
chip goes. The host keeps the label and the summary's words (what a
section counts).

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

Two panes split from one screen, each in a pane frame with a title laid
out for the room its border leaves, the first focused:

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use tuikit::frame::{PaneFrame, SplitPane};
use tuikit::testkit::buffer::rows;
use tuikit::text;
use tuikit::theme::Theme;

let theme = Theme::default();
let area = Rect::new(0, 0, 48, 6);
let mut buf = Buffer::empty(area);
let split = SplitPane::new(40);
let [list, detail] = split.areas(area);
let focused = 0;
for (index, (pane, title)) in [(list, "a list of things"), (detail, "the detail")]
    .into_iter()
    .enumerate()
{
    let title = text::truncate_end(title, PaneFrame::title_width(pane.width));
    let inner = PaneFrame::new(&theme)
        .title(Line::from(title))
        .focused(index == focused)
        .render(pane, &mut buf);
    assert_eq!(inner, Rect::new(pane.x + 1, 1, 22, 4));
}
let text = rows(&buf);
assert_eq!(text[0], "┌ ► a list of things ──┐┌   the detail ────────┐");
assert_eq!(buf[(0, 5)].fg, theme.panel_focus_accent, "the focused border");
assert_eq!(buf[(24, 5)].fg, ratatui::style::Color::Reset, "the other border");

// Below the threshold the panes stack.
assert!(split.stacks(Rect::new(0, 0, 39, 6)));
```

Two sections of one pane, each headed by a rule laid out at the width it
is drawn in: a list of links with its counts before the chip, and a
preview with no summary. Their chips end in the same column.

```rust
use ratatui::text::Line;
use tuikit::frame::SectionRule;
use tuikit::text;
use tuikit::theme::Theme;

let theme = Theme::default();
let width = 36;
let links = SectionRule::new("Links")
    .summary("3 shown · 1 more")
    .summary_first()
    .line(width, &theme);
let preview = SectionRule::new("Preview").line(width, &theme);
assert_eq!(links.to_string(), "───────── 3 shown · 1 more  Links ──");
assert_eq!(preview.to_string(), "───────────────────────── Preview ──");

let chip_end = |line: &Line, label: &str| {
    let drawn = line.to_string();
    text::width(&drawn[..drawn.find(label).expect("chip") + label.len() + 1])
};
assert_eq!(chip_end(&links, "Links"), width - SectionRule::CLOSING);
assert_eq!(chip_end(&preview, "Preview"), width - SectionRule::CLOSING);
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `ModalFrame::new(&theme, size)` | a frame sized by a policy | no title, no padding |
| `title(line)`, `padding(padding)` | set the title and the padding | the title's spans keep their styles (chips); padding is ratatui's `Padding` |
| `footer(line)` | set a line on the bottom border | the title's form; for a modal whose body has no row to spare for its keys (a one-line prompt), the line `ModalFooter::border` gives; the modal lays it out for `inner_width` less 2 so it breaks between hints, and the frame cuts what still does not fit at the corner |
| `inner_width(area)` | the columns a body gets | the policy's width less the border and padding |
| `rect(area, rows)` | the frame's rect for a body of `rows` rows | centered; a fill policy ignores `rows` |
| `render(area, buf, body)` | measures the body once, draws the frame, draws the body | returns the body's `Output` |
| `ModalBody::measure(width)`, `render(area, buf)` | the body's two calls | `measure` once per draw, before the frame is placed |
| `Lines::new(lines)`, `following(rows)` | a body of lines laid out ahead, scrolled to keep `rows` in view | one row per line; returns the window it showed |
| `ModalFooter::new(footer, &theme)`, `.status(line)` | a modal's footer, and the line kept above its keys | the keys of a `keys::Footer` (a modal takes its table's from `Footer::leaving_last`); the status line is a form's error line or a reader's find prompt, in its own style |
| `.keys(width)`, `.height(width)` | the key rows in the footer's look; the rows the footer asks for | broken only between segments; the height counts the blank row and the status line |
| `.fit(width, room)`, `.fit_beside(area, keep, body_rows)` | the footer fitted to the rows the body leaves | a `FooterFit`; `fit_beside` leaves the body `keep` rows and lays everything out a cell narrower when the body overflows beside a scrollbar (`ScrollGutter::carves`: not on a body shorter than the bar's five rows) |
| `.border(width)` | the footer as a bottom border shows it | its last key row, in the footer's look |
| `FooterFit { kept, spill, width }`, `.split(area)`, `.render(area, buf)` | the rows on the frame's last rows, the rows that scroll after the body, the width they were laid out at | `split` gives the body's rows and the footer's; `render` draws the kept rows |
| `ModalFooter::style(&theme)` | the footer's look | the theme's `muted`, no color |
| `ModalSize { width, height, fit }` | a policy | `width(screen)`, `height(screen, wanted)`, `rect(area, wanted)` |
| `Extent::new(cap, min, margin)`, `share(percent)` | one direction of a policy | `cells(screen, wanted)` applies the rule |
| `Fit::Content`, `Fit::Fill` | where the height comes from | the body's rows, or all the extent allows |
| `PaneFrame::new(&theme)` | an unfocused pane frame with no title | |
| `.title(line)`, `.focused(bool)` | set the title and whether the pane has focus | the title's spans keep their styles; focus tints the border and shows the marker |
| `PaneFrame::title_width(width)` | the cells a title may take in a pane `width` wide | the border between its corners less `LEAD`; the closing space is not counted |
| `PaneFrame::LEAD` | the lead's cells | 3: a space, the marker's cell, a space |
| `.inner(area)`, `.render(area, buf)` | the area inside the border; draw the border and title | `render` returns the inner area |
| `SplitPane::new(threshold)` | a split that stacks below `threshold` columns | `threshold()` reads it back |
| `.stacks(area)`, `.areas(area)` | whether the panes stack on `area`; the two panes' areas | first then second, halves, the odd cell to the first |
| `SectionRule::new(label)` | the rule of the section named `label` | no summary |
| `.summary(text)`, `.summary_first()` | show a summary beside the chip, after it or before it | an empty summary shows nothing |
| `.line(width, &theme)` | the rule `width` cells wide | never wider: the summary, then the label, give way |
| `SectionRule::CLOSING` | the closing rule's cells | 2 |

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
| `MENU` | up to 64, at least 40 | the body's, at least 3 | 2 columns, 1 row | a list of choices, a settings list, a dialog |
| `FORM` | up to 80, at least 44 | the body's, at least 3 | 2 columns, 1 row | labeled fields |
| `WIDE` | up to 100, at least 40 | all, at least 10 | 2 columns, 1 row | reading (help, a log, a long value); a long list that is filtered |
| `PALETTE` | 85% of the screen, 60 to 200 | 80%, 12 to 50 | 2 columns, 1 row | a palette (a filter picker, a search), which shows more of each result on a large screen |

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

### The footer

Every framed control (the menu, the dialog, the checklist, the settings
panel, the form, the palette, the framed pager, the log list, the prompt)
draws its keys through `ModalFooter`, so a footer looks and fits the same
in every modal:

```text
│► Open                  │   the body
│  Rename                │
│                        │   the blank row
│an error or find line   │   the status line, when the modal has one
│↑/↓ move · Enter pick · │   the keys, receding, broken between segments,
│Esc close               │   the keys that leave last, on the last rows
└────────────────────────┘
```

- **Look.** Every segment is drawn in `ModalFooter::style`: the theme's
  `muted`, regular weight, and no color of its own, so it takes the
  body's foreground (or the accent, on a bottom border). A segment that
  cannot act now keeps `keys::unavailable_style`. The status line keeps
  its own style (an error line is `error`).
- **Order.** A footer lists the keys that leave last: a control takes its
  table's footer from `keys::Footer::leaving_last`, which keeps the other
  rows and the notes in table order and puts the rows that leave or
  dismiss after them.
- **Fit.** The body keeps at least one row (or the rows its control says,
  such as a palette's query). When the footer wants more than the rest:
  the blank row goes first; then the status line takes a row while there
  is one; then the keys keep their last rows, and their first rows spill.
  A body draws the spilled rows (a blank row and the first key rows) after
  its last row, where they scroll with it, so no key is lost and the keys
  that leave are the last to go out of view. A footer spills only when
  the body is down to the rows it keeps, so a list whose window follows
  its cursor shows none of them, while a body that scrolls freely (a
  reader, a dialog's body, a log's detail) brings them into view. When the
  body and the spilled rows overflow beside a scrollbar, `fit_beside`
  lays the footer out a cell narrower so its rows line up beside the bar.
- **The bottom border.** A modal with no row to spare (the one-line
  prompt) hands `ModalFrame::footer` the line `ModalFooter::border` gives:
  the last key row at the border's width less a space on each side, in
  the footer's look over the accent. With no body to scroll into, the
  first rows are not shown.
- **Very narrow frames.** At eight cells or fewer the keys stay on one
  row, cut at the frame's edge.

A host's own modal does the same in a few lines: lay its body out, then

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Widget};
use tuikit::frame::ModalFooter;
use tuikit::keys::{Footer, MENU_KEYS};
use tuikit::testkit::buffer::rows;
use tuikit::theme::Theme;

let theme = Theme::default();
let area = Rect::new(0, 0, 14, 3);
let mut buf = Buffer::empty(area);
let entries = vec![Line::from("Open"), Line::from("Rename")];
let footer = ModalFooter::new(Footer::leaving_last(MENU_KEYS), &theme);
// Three rows: the body keeps one, and the footer's three key rows and
// its blank row want more than the two left; the blank row and the
// first key row give way, the keys that leave stay.
let mut fit = footer.fit_beside(area, 1, |_| entries.len());
let [body, keys] = fit.split(area);
let mut lines = entries;
lines.append(&mut fit.spill);
Paragraph::new(lines).render(body, &mut buf);
fit.render(keys, &mut buf);
assert_eq!(rows(&buf)[0].trim_end(), "Open");
assert_eq!(rows(&buf)[1].trim_end(), "Enter pick");
assert_eq!(rows(&buf)[2].trim_end(), "Esc close");
```

### `PaneFrame`

The top border of a pane `width` cells wide holds, between its corners:

```text
┌ ► title ──────────┐   focused: the border and ► in panel_focus_accent
┌   title ──────────┐   unfocused: the plain border, a blank marker cell
┌ ► a title that fil┐   a title wider than its room is cut at the corner
```

- The lead is three cells (`LEAD`): a space, the marker's cell, a space.
  The blank that stands for the marker while the pane has no focus keeps
  the marker's style, so the two cells differ only in their symbol as
  focus moves.
- The title follows in its own spans. A reset style sits under it, so
  the border's tint never colors the title's text; only the marker and
  the border carry the cue.
- The closing space is drawn only when the lead and the title leave a
  cell before the corner, so a title laid out for `title_width` exactly
  ends at the corner and a shorter one ends with a space.
- With no title text, a focused pane still shows the lead with its
  marker; an unfocused one draws an unbroken top border.
- The frame does not clear its area or paint a background: a pane is the
  screen itself, and what it shows fills its inner area.

A host that keeps several panes passes `focused(true)` to the one that
has the keys; the others draw plain. The framework's tree pane
([tree_view.md](tree_view.md)) draws itself in a pane frame and passes
its own focus.

### `SplitPane`

The split is two halves of one area, by direction:

- At or above the threshold, side by side: the first pane on the left,
  the second on the right, each half the width and the full height.
- Below it, stacked: the first on top, the second below, each the full
  width and half the height.
- An odd width or height gives its odd cell to the first pane.

`stacks(area)` says which a given area gets, for a host whose layout of
a pane's insides depends on it. The split draws nothing; the panes are
the host's, usually each in a `PaneFrame`.

### `SectionRule`

A section rule is one line, laid out for the width it is drawn in:

```text
──────────────────────── Label  summary ──    the summary after the chip
──────────────────────── summary  Label ──    summary_first
─────────────────────────────────── Label ──  no summary
```

- The chip is the label with a space on each side, in `accent` under
  `badge` (a filled pill), at the label's own width.
- The summary, when there is one, has a space on each side, in `inert`.
  After the chip by default; `summary_first` puts it before the chip, so
  the chip sits against the closing rule in the same column whatever the
  summary says, which keeps the labels easy to find down a pane whose
  counts change.
- The closing rule is `CLOSING` (two) cells of `marks::RULE`; the leading
  rule fills what the chip, the summary, and the closing rule leave, all
  measured in terminal cells, and drops out when they leave nothing.
- Both rules are in `muted`.

The line is never wider than its width. Below the width of the chip, the
summary, and the closing rule, the pieces give way in turn, and the
closing rule stays:

1. The summary is cut with `…` (a cut that ends at a space drops it, so
   the mark follows the last word kept, and the leading rule takes the
   cell), and leaves with its spaces once not one character would show
   before the mark.
2. Then the label is cut the same way, the chip keeping its spaces.
3. Then the chip leaves, and the line is a plain rule of its width in
   `muted` (no rule at all at width 0).

```text
──────────── 3 shown · 1 more  Links ──   the whole width
─ 3 shown…  Links ──                       20 cells: the summary is cut
─ Links ──                                 10 cells: the summary has left
 Li… ──                                     7 cells: the label is cut
───                                         3 cells: a plain rule
```

So a rule never folds onto a second row or runs under a pane's border.
Lay a rule out at the width it is drawn in; in a list that may scroll,
that is the width the scrollbar leaves (see
[master_detail.md](master_detail.md#laying-the-list-out)).

## Key table

None: the frames handle no keys. The modal inside a modal frame has its
own key table, and its footer is part of its body or, for a body with no
row to spare, the line the modal hands `footer`. A pane's keys are its
host's (which pane has focus, and what moves it, is the host's key
table); the pane frame only shows the result. A split and a section
rule handle no keys either.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | a modal's border, and so its title and footer, which take the border's color; a section rule's chip |
| `panel_focus_accent` | a focused pane's border and its focus marker |
| `surface` | the background painted over a modal's cleared rect |
| `badge` | the modifier that fills a section rule's chip |
| `muted` | a section rule's leading and closing rules; every modal footer (`ModalFooter`) |
| `inert` | a section rule's summary |

A title's chips (a segmented choice) keep the look their own module gave
them, from `badge` and `accent`. The body styles itself. A pane's title
styles itself too, and an unfocused pane's border uses the terminal's own
look.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 8 | Footers and hints | The frame draws no hint of its own; a modal's footer, generated from its key table, is drawn by `ModalFooter` in one look and fitted by one rule, in its body or on its bottom border |
| 9 | Focus cue | A focused pane's border and title marker take `panel_focus_accent`; an unfocused pane keeps the plain border and a blank where the marker goes, so its title stays in place |
| 13 | Theme | A modal's border and background come from `accent` and `surface`, a focused pane's border and marker from `panel_focus_accent`, a section rule's pieces from `muted`, `accent`, `badge`, and `inert`; nothing names a color |
| 14 | Width | A body is laid out for the inner width the policy gives in cells, so nothing is laid out for one width and drawn at another; a pane's title is laid out for `title_width`, in cells; a section rule measures its chip and summary in cells, fills the width it is given, and cuts its summary and then its label with `…` so it never runs past it |

Not applicable: 1 to 7, 10 to 12, and 15 to 18 govern how lists move,
and how modals confirm, close, edit, and page. The frames only place and
draw boxes, a split only places panes, and a section rule only draws a
line; the modals, panes, and lists inside them follow those rules.

## Seams left open

- **A split's ratio.** The halves are fixed. A share for the first pane,
  or one that widens whichever pane has focus, would be a parameter of
  `SplitPane`; every caller would still get both areas from it.
- **More than two panes.** A split of three or more, or a split nested in
  a pane, is a `SplitPane` over one of the areas a split returned today;
  a list of shares would make it one call.
- **Focus under an overlay.** The pane frame shows whatever focus its
  host passes. A host that wants no pane framed in color while a modal
  has the keys passes `focused(false)` while one is open; the frame needs
  nothing more.
- **A pane's background.** A pane does not paint `surface`; a host that
  wants panes on a colored ground would read a pane token here.
- **Mouse hit testing.** `rect(area, rows)` is the rect the frame drew
  for the same rows, which a click on or outside the modal would be
  tested against; a split's `areas` are the rects a click picks a pane
  by.
- **Border looks.** The border is plain lines; a theme token naming a
  border set (rounded, thick, double) would let a host change every
  frame at once.
- **Title alignment.** Titles sit at the left; a centered or right title
  would be a parameter of the one title form, not a caller's choice of
  padding.
