# `selection`

| | |
| --- | --- |
| Module | `tuikit::selection` |
| Layer | 1 (foundation) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: `Ends::step` returns the moved index, and the row functions return or restyle spans |

## Purpose

`selection` is how every list shows its cursor row and what its cursor
does at the ends of the list. A menu, a picker, a checklist, a settings
list, a form's fields, a palette's results, a log, a tree, and a
property list all show the cursor the same way:

- **The marker.** Every row starts with a 2-cell gutter: the pointer
  mark (`marks::POINTER`) and a space on the cursor row, two spaces on
  every other row. Text after the gutter never moves as the cursor does,
  and the marker alone tells the cursor row apart on a terminal that
  draws no color and no reverse video.
- **The highlight.** The cursor row takes the theme's selection pair
  from its gutter to its last span: `selection_active` while the list
  has focus, `selection_inactive` while it does not. An unfocused list
  keeps its marker and its inactive look, so it always shows where its
  cursor is. A filled span (a chip, an input field's fill: anything with
  a background or drawn reversed) keeps the look it has on every other
  row, so it still reads as a chip or a field inside the highlight. The
  row's own colors and its muted text stay as they are under the
  highlight.
- **Ghost and pending.** A row that stands for something gone, stopped,
  or not started wears the ghost look: the `inert` color in italics, its
  bold dropped, its chips kept filled and receding. A row that keeps its
  old values while a change it waits for lands recedes (`muted`). Both
  are applied before the highlight, so a selected ghost row is still a
  ghost.
- **The ends.** `Ends::Wrap` for short choice lists (menus, pickers,
  checklists, palettes, settings lists, a form's ring of fields): past
  the last row comes the first. `Ends::Clamp` for scrollable content
  (trees, property lists, pagers, logs): the cursor stops at the first
  and last rows. A list picks its rule by what it is, not by how long it
  happens to be.

The module replaces what a program grows one list at a time: a cursor
glyph per family (a pointer, an ASCII `>`, a third-party widget's own
indicator, none), a highlight per family (bold only, a hand-written
reverse, an accent-colored glyph only, the theme's pair in two places),
a list that highlights only one span of its cursor row, an unfocused
pane whose cursor vanishes, and menus that stop at their ends beside
pickers that wrap. The host keeps what a row says, where its cursor is,
which kind of list it is, and whether the list has focus.

## Usage example

A short menu and a long log, each stepping by its own rule and drawing
every row through `row`. The log is unfocused, so its cursor row takes
the inactive look and keeps its marker.

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tuikit::glyphs::marks;
use tuikit::selection::{self, Cursor, Ends};
use tuikit::theme::Theme;

struct List {
    rows: Vec<String>,
    cursor: usize,
    ends: Ends,
}

impl List {
    fn step(&mut self, delta: i32) {
        self.cursor = self.ends.step(self.cursor, self.rows.len(), delta);
    }

    fn lines(&self, focused: bool, theme: &Theme) -> Vec<Line<'_>> {
        self.rows
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let cursor = Cursor::of(index == self.cursor, focused);
                selection::row([Span::raw(text.as_str())], cursor, theme)
            })
            .collect()
    }
}

let theme = Theme::default();
let mut menu = List {
    rows: vec!["Open".into(), "Rename".into(), "Close".into()],
    cursor: 0,
    ends: Ends::Wrap,
};
let mut log = List {
    rows: (1..=40).map(|n| format!("entry {n}")).collect(),
    cursor: 0,
    ends: Ends::Clamp,
};
menu.step(-1);
log.step(-1);
assert_eq!(menu.cursor, 2, "the menu wraps to its last row");
assert_eq!(log.cursor, 0, "the log stops at its first row");

let area = Rect::new(0, 0, 20, 3);
let mut buf = Buffer::empty(area);
Paragraph::new(menu.lines(true, &theme)).render(area, &mut buf);
assert_eq!(buf[(0, 2)].symbol(), marks::POINTER);
assert_eq!(buf[(2, 2)].symbol(), "C");
assert_eq!(buf[(2, 1)].symbol(), "R", "every row keeps its text column");
assert!(buf[(2, 2)].modifier.contains(Modifier::REVERSED));

let unfocused = log.lines(false, &theme);
assert_eq!(unfocused[0].spans[0].content, format!("{} ", marks::POINTER));
assert_eq!(unfocused[0].spans[1].style, Style::default().add_modifier(theme.selection_inactive));
```

A row with its own styling, a chip, and the ghost look:

```rust
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use tuikit::chip::Chip;
use tuikit::selection::{self, Cursor};
use tuikit::theme::Theme;

let theme = Theme::default();
let mut spans = vec![
    Span::styled("build", Style::default().add_modifier(Modifier::BOLD)),
    Span::raw(" "),
    Chip::new("job").color(Color::Magenta).span(&theme),
];
// Gone: the ghost look first, then the row and its highlight.
selection::ghost(&mut spans, &theme);
let line = selection::row(spans, Cursor::Active, &theme);
assert_eq!(line.spans[1].style.fg, Some(theme.inert));
assert!(line.spans[1].style.add_modifier.contains(Modifier::ITALIC | theme.selection_active));
assert_eq!(line.spans[3].style.fg, Some(Color::Magenta), "the chip keeps its fill");
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `Cursor::{Off, Active, Inactive}`, `Cursor::of(on, focused)` | whether the cursor is on a row and its list has focus | `is_on`, `modifier(theme)`, `style(theme)` |
| `GUTTER_WIDTH` | the gutter's cells | 2 |
| `gutter(cursor)` | the pointer mark and a space, or two spaces | unstyled |
| `highlight(spans, cursor, theme)` | patches the selection pair onto every span | filled spans keep their look; off the cursor nothing changes |
| `row(spans, cursor, theme)` | the gutter, then the spans, highlighted | the one call a list makes per row |
| `ghost_style(theme)`, `ghost(spans, theme)` | the ghost look | apply before `highlight` |
| `pending(spans, theme)` | the receding look of a row waiting for a change | apply before `highlight` |
| `Ends::{Wrap, Clamp}`, `Ends::step(index, len, delta)` | moves a cursor by the list's rule | `scroll::wrap_step` or `scroll::clamp_step`; 0 for an empty list |

### Which lists wrap

| Kind of list | Ends | Why |
| --- | --- | --- |
| menus, pickers, checklists, palettes, settings lists | `Wrap` | short: the far end is one step away |
| a form's fields (`Tab`/`Down`, `Shift-Tab`/`Up`) | `Wrap` | one ring (house rule 6) |
| trees, property lists, explorers | `Clamp` | long: running off the end should not jump to the start |
| pagers, logs, message lists | `Clamp` | the ends are where the reader stops |

A value that cycles in place (a toggle's states, the choices inside one
form field, a strip of tabs) has no cursor row; it steps with
`scroll::wrap_step` directly. Page keys move by the window's page and
stop at the ends whatever the list's rule (house rule 12).

### Where the marker goes

A list whose rows already start with two blank cells (an indent, a lead
before a label) puts the gutter there, so adding the marker moves no
column. A list whose first cells carry meaning of their own (a tree's
disclosure arrows and kind glyphs) may draw the highlight alone with
`highlight` until its columns make room for the gutter; the highlight
still follows the focus.

## Key table

None: the module handles no keys. A list maps `Up`/`Down` (and the
aliases its key table gives) to `Ends::step` with a delta of 1, and
`Home`/`End` to the first and last row.

## Theme tokens

| Token | Used for |
| --- | --- |
| `selection_active` | the cursor row of the focused list |
| `selection_inactive` | the cursor row of an unfocused list |
| `inert` | the ghost look's color |
| `muted` | the pending look, and a ghost row's chips |

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | `row` draws the pointer in a 2-cell gutter and highlights the row from the theme's pair; a colorless theme still shows the marker |
| 2 | List ends | `Ends::Wrap` for short choice lists, `Ends::Clamp` for scrollable content |
| 10 | Unfocused cursor | `Cursor::Inactive` keeps the marker and takes `selection_inactive` |
| 13 | Theme | every look comes from the theme's tokens; the raw-style guard fails on a hand-written reverse in framework source |
| 14 | Width | the gutter is two cells, measured, whatever the mark |

Not applicable: 3 to 9, 11, 12, and 15 to 18 govern confirmation,
closing, forms' keys, errors, footers, the focus cue on a pane's border,
feedback, paging, editing, paths, labels, and pagers. The module draws
rows and steps cursors; the controls that use it follow those rules.

## Seams left open

- **A highlight to the edge.** The highlight covers the spans a row is
  made of. A list that wants a bar across its full width pads its last
  span to the width; a width argument on `row` would do it for every
  list.
- **Disabled rows.** A choice that cannot be picked today is left out of
  its list. A disabled look (and a step that skips such rows) would join
  `Cursor` and `Ends`.
- **More than one selected row.** A checklist marks its checked rows
  with its own marks; a multi-row selection (a range, a set) would need
  a look of its own beside the cursor's.
- **Mouse hit testing.** A click maps to a row through the window's
  visible range and sets the cursor; the look needs nothing more.
- **Cursor glyph per host.** The marker is the framework's pointer mark.
  A host that wanted another would register a mark of its own; `gutter`
  would take it from the theme.
