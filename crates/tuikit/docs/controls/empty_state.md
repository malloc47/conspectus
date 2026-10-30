# `EmptyState`

| | |
| --- | --- |
| Module | `tuikit::empty_state` |
| Layer | 1 (component) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | none: it draws rows, or returns them for a region that lays out its own |

## Purpose

A list with no rows, a detail pane with nothing selected, and a preview
whose content has not loaded each fill their slot with a sentence instead
of leaving it blank: what is missing, and often what would fill it
(`Nothing matches "draft".` then `Press s to search again.`). Written at
each site, those sentences drift: one wraps and another is cut by the
border, one recedes and another reads like content, one spells its second
line into the first. `EmptyState` gives them one look: a message and an
optional hint, wrapped at word edges, in the theme's `muted` look,
left-aligned from the slot's top-left corner or centered in the slot.

The host keeps the words: which message and hint fit which state, in its
own vocabulary, with the keys it binds. A slot that shows a failure (a
reason something could not load) is content, not an empty state; it keeps
the look its host gives failures.

## Usage example

A list's slot with nothing to list, then the same slot centered.

```rust
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuikit::empty_state::EmptyState;
use tuikit::testkit::buffer::rows;
use tuikit::theme::Theme;

let theme = Theme::default();
let empty = EmptyState::new("No entries match `size:>1G`.")
    .hint("Press F to clear filters, f to edit.");

// From the top-left corner, wrapped to the slot.
let area = Rect::new(0, 0, 24, 4);
let mut buf = Buffer::empty(area);
empty.render(area, &mut buf, &theme);
assert_eq!(rows(&buf)[0], "No entries match        ");
assert_eq!(rows(&buf)[1], "`size:>1G`.             ");
assert_eq!(rows(&buf)[2], "Press F to clear        ");
assert_eq!(rows(&buf)[3], "filters, f to edit.     ");
assert_eq!(buf[(0, 0)].modifier, theme.muted);

// Centered across and down a slot with nothing around it.
let area = Rect::new(0, 0, 30, 5);
let mut buf = Buffer::empty(area);
EmptyState::new("Loading…").centered().render(area, &mut buf, &theme);
assert_eq!(rows(&buf)[2], "           Loading…           ");

// Rows for a region that lays out lines of its own.
let lines = EmptyState::new("no preview for this item").lines(40, &theme);
assert_eq!(lines.len(), 1);
```

A tree's pane takes one for when no row shows:
`TreeView::new(..).empty(EmptyState::new(..).hint(..))`, or a string, which
becomes a message with no hint (see [tree_view.md](tree_view.md)).

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `EmptyState::new(message)` | a state that says `message` | left-aligned, no hint; also `From<&str>` and `From<String>` |
| `.hint(text)` | add a hint on its own rows after the message | the action that would fill the slot, or what happens next |
| `.centered()`, `.align(Align)` | place the rows | `Align::Left` (default) or `Align::Center` |
| `.message()`, `.hint_text()`, `.text()` | the words; both joined by a line break | for a reader that takes plain text |
| `.rows(width)` | the wrapped rows, centered when asked | strings, for counting and tests |
| `.lines(width, &theme)` | the rows as lines in the `muted` look | for a scrolled body or a list's window |
| `.render(area, buf, &theme)` | draw the state in `area` | the whole area takes the `muted` look |

### The look

- **Rows.** The message wrapped by `text::wrap`, then the hint wrapped the
  same way: at word edges, a word wider than the slot broken between
  graphemes, no row wider than the width. A line break in either starts a
  row.
- **Placement.** Left-aligned rows start in the slot's first column and
  the first row is its top. Centered rows are each centered across the
  width (spaces a row keeps before its break are not counted), and in an
  area the block is centered down it too; a slot shorter than the rows
  draws the first rows that fit.
- **Look.** Message and hint both in the theme's `muted` look, and when
  drawn in an area, the whole area takes it, so the slot reads as one
  receding block.

## Key table

None: the state handles no key. Its hint names keys the host binds in its
own tables.

## Theme tokens

| Token | Used for |
| --- | --- |
| `muted` | the message, the hint, and the area they are drawn in |

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 13 | Theme | takes `&Theme` and draws in its `muted` token; no color is named |
| 14 | Width | the rows are wrapped and centered in terminal cells through `text`; no row runs past the slot for its border to clip |

Not applicable: 1, 2, and 10 (no list or cursor), 3 to 8, 12, 15, 16, and
18 (no keys, no overlay, no editing, no paging), 9 (the pane around the
state carries the focus cue), 11 (an empty state is not feedback; a
failure keeps its host's look), and 17 (the words are the host's).

## Seams left open

- **A screen too small to draw.** A host (or `AppFrame`, below a size the
  host names) would draw one centered `EmptyState` over the whole screen
  saying how much room it needs; see [app_frame.md](app_frame.md#seams-left-open).
- **A look for the hint.** Message and hint share the muted look. A hint
  set apart (its keys in the accent, as a help page draws them) would be a
  token here.
- **An illustration or a title.** Some programs lead an empty state with a
  glyph or a bold title line; either would be a row before the message.
- **An action the hint names.** The hint is words; a host that wants the
  key in the hint to work while the state shows binds it in its own table.
