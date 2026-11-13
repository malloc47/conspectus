# `PropertyGrid`

| | |
| --- | --- |
| Module | `tuikit::property_grid` |
| Layer | 1 (component) |
| Type parameters | none: rows are data (text, the host's styled spans and glyphs) |
| Cargo feature | none |
| Outcomes | `Intent`: `Copy { label, text }`, `Inspect { label, text }`, `Toggle`, `Follow`, or none, from `activate` (the host's `Enter`) and `inspect` (the host's inspect key) |

## Purpose

A detail pane lists what it knows about the thing it shows: its
properties (a label and a value), the links that lead from it to other
things, and sections that fold some of those links away. Written by
hand, each row kind pads its own label column, cuts its value in the data
that builds it, highlights a part of its row, types its marks, and copies
or opens a value by looking the row up a second time. A property grid
states the three kinds of row once:

- **`PropertyRow`**: the label in a fixed label column (bold; a label as
  wide or wider keeps one blank cell before its value), an optional glyph
  for the kind of thing the value names, then the value, as text or the
  host's spans. A value can be cut to a number of cells at its end or in
  its middle; a cut value draws a hint after it, `(truncated · o)` with
  the key the host names, and the whole value stays in the row for a copy
  or an inspect. A row with no value (a stand-in such as the no-value
  mark) recedes and has nothing to copy. An annotation that needs
  attention ends the row.
- **`LinkRow`**: `verb  glyph  label`: an indent, the verb in a fixed
  verb column, a one-cell glyph for the far end's kind (the no-value mark
  when there is none), and the far end's label cut in the middle to the
  room left, so the row never wraps and both ends of a path stay
  readable. Meta follows the label; the label leaves it room down to
  `MIN_LINK_LABEL` cells, and past that the meta runs on for a host that
  wraps its lines to carry. A `Tone` says how much the link holds:
  primary, quiet (an alternative), alert (a conflict, led by the warning
  mark, so the signal survives a terminal with no color), or inert (no
  known far end: the whole row recedes and following it does nothing).
- **`DisclosureRow`**: a section header the cursor can stand on: the
  disclosure mark in the theme's `disclosure` color (the mark and look a
  tree's folding rows have), a label, and a summary of how many rows the
  section holds and how many of them carry a mark, `(4 · 2 ▲ · 1 —)`.

`PropertyGrid` holds the rows in order, with lines that are no row (a
rule, a heading) between them, and draws every row through the shared
cursor look, whole, focused or not. Its cursor counts rows only.

The host keeps what each row says (which fields a thing has, the verb
for a link, which tone a link takes, which glyph marks its far end, the
label of its inspect key), the cursor and which rows a closed section
hides, the keys, and every effect: the clipboard sink a copy goes to, the
reader an inspected value opens in, the fold, and the move to a link's
far end.

## Usage example

A file's details: a few properties, one cut, two links, and a folded
section with an alternative under it. The host draws the grid, then asks
what `Enter` and its inspect key mean on the cursor's row and does it.

```rust
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use tuikit::columns::Cut;
use tuikit::glyphs::marks;
use tuikit::property_grid::{
    DisclosureRow, Intent, LinkRow, PropertyGrid, PropertyRow, Tone, Tones,
};
use tuikit::theme::Theme;

let theme = Theme::default();
let kind = Span::styled("#", Style::default().fg(Color::Magenta));
let mut grid = PropertyGrid::new()
    .inspect_key("o")
    // An alternative in the host's own role instead of the theme's `inert`.
    .tones(Tones::new(&theme).with(Tone::Quiet, Style::default().fg(Color::Gray)))
    .row(PropertyRow::new("name", "report.txt"))
    .row(PropertyRow::new("path", "/srv/archive/2026/q3/report.txt").cut(Cut::Middle, 20))
    .row(PropertyRow::new("owner", marks::NO_VALUE).placeholder());
grid.push_line(Line::raw("-- links --"));
grid.push(LinkRow::new("stored in", "/srv/archive").glyph(kind.clone()));
grid.push(DisclosureRow::new("Other", true).count(1));
grid.push(LinkRow::new("copied from", "/tmp/report.txt").glyph(kind).indent(4).tone(Tone::Quiet));

// The cursor is on the cut path; the grid has focus.
let cursor = 1;
let drawn = grid.lines(60, Some(cursor), true, &theme);
assert_eq!(drawn.cursor_line, Some(1));
let text: Vec<String> = drawn.lines.iter().map(ToString::to_string).collect();
assert_eq!(text[1].trim_end(), "► path          /srv/archi…eport.txt  (truncated · o)");
assert_eq!(text[3], "-- links --", "a line that is no row is drawn as given");

// The host does what the row asks.
match grid.inspect(cursor) {
    Some(Intent::Inspect { label, text }) => {
        // Open `text` whole in a reader titled `label` (a `Pager` modal).
        assert_eq!((label.as_str(), text.as_str()), ("path", "/srv/archive/2026/q3/report.txt"));
    }
    other => panic!("{other:?}"),
}
match grid.activate(cursor) {
    // Write the whole value to the clipboard sink, and say so.
    Some(Intent::Copy { text, .. }) => assert!(text.ends_with("q3/report.txt")),
    other => panic!("{other:?}"),
}
assert_eq!(grid.activate(2), None, "a placeholder has nothing to copy");
assert_eq!(grid.activate(3), Some(Intent::Follow));
assert_eq!(grid.activate(4), Some(Intent::Toggle), "the host folds the section");
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `PropertyRow::new(label, text)`, `PropertyRow::spans(label, spans)` | a property with a text or a styled value | `style` styles a text value |
| `.cut(Cut::End \| Cut::Middle \| Cut::Clip, cells)` | shows at most `cells` of the value | a cut value draws the truncated hint; spans are cut at their end |
| `.placeholder()`, `.hint(text)`, `.annotation(text)`, `.glyph(span)` | a stand-in value; a note after the value (replaces the truncated hint); a note in `warning`; a glyph before the value | |
| `text()`, `shown()`, `is_cut()`, `activate()`, `inspect()` | the whole value, the value as drawn, whether it is cut, the copy, the inspect | |
| `LinkRow::new(verb, label)` with `.indent(cells)`, `.glyph(span)`, `.meta(text)`, `.tone(tone)` | a link | an alert row's mark takes the indent's first two cells |
| `Tone::{Primary, Quiet, Alert, Inert}`, `Tone::style(theme)` | how much a link holds | `Tone::ALL` lists them |
| `Tones::new(theme)`, `.with(tone, style)`, `.style(tone)` | the look of each tone | from the theme's tokens, or a host role per tone |
| `DisclosureRow::new(label, open)` with `.count(n)`, `.tally(n, mark)` | a section header | `summary()`; a tally of none is left out |
| `Row::{Property, Link, Disclosure}` | one row of a grid | `From` each row type; `activate`, `inspect` |
| `PropertyGrid::new()` with `.label_width`, `.verb_width`, `.inspect_key`, `.tones`, `.row` | a grid | `LABEL_WIDTH` 14, `VERB_WIDTH` 22 unless told |
| `push(row)`, `push_line(line)`, `len`, `get`, `activate(index)`, `inspect(index)` | rows, lines that are no row, and the intents by row index | the index counts rows only |
| `lines(width, cursor, focused, theme)` | every line drawn, and `GridLines::cursor_line` | `width` lays out the link rows |

### What wraps and what does not

Property rows and section headers are drawn whole. A host that wraps its
lines to a narrow area (the scroll module's wrapped rows) wraps a long
value onto the rows under it, and the glyph stays beside the label. Link
rows are laid out in the width the grid is drawn at and do not wrap: the
label gives way, cut in the middle; only meta the label could not leave
room for runs past the width.

## Key table

None: the grid handles no keys. Its host maps `Enter` to
`PropertyGrid::activate` and the key it names with `inspect_key` to
`PropertyGrid::inspect` on the cursor's row, steps the cursor with
`selection::Ends::Clamp` (a property list is scrollable content), and
does what the returned `Intent` asks. The explorer
([explorer.md](explorer.md)) is such a host: its table,
`EXPLORER_KEYS`, binds `Enter`, the inspect key `o`, and the fold keys
over grid rows, and turns each intent into its outcome.

## Theme tokens

| Token | Used for |
| --- | --- |
| `selection_active`, `selection_inactive` | the cursor row, whole, through `selection::row` |
| `muted` | a placeholder value, a property's hint, an inert link row |
| `warning` | a property's annotation; the alert tone |
| `link` | the primary tone |
| `inert` | the quiet tone; a section header's label and summary |
| `disclosure` | a section header's mark, as a tree's folding mark |

The host replaces any tone's look with a role of its own through
`Tones::with`; the grid reads no named style itself.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | every row goes through `selection::row`: the pointer in the 2-cell gutter and the selection pair over the whole row |
| 2 | List ends | the host steps a grid's cursor with `Ends::Clamp`: it is scrollable content |
| 10 | Unfocused cursor | `lines(.., focused: false, ..)` keeps the marker in the inactive pair |
| 13 | Theme | every look is a theme token or a style the host passes; no raw colors |
| 14 | Width | the label column, the verb column, and every cut measure cells; a link label is cut in the middle, a value at the end or in the middle as the row asks, with `…` |

Not applicable: 3 to 9, 11, 12, and 15 to 18 govern confirmation,
closing, forms, errors, footers, the pane's focus cue, feedback, paging,
editing, paths, labels, and pagers. The grid draws rows and reports
intents; the host that owns its cursor and keys follows those rules (the
reader an inspected value opens in follows rule 18).

## Seams left open

- **A measured label column.** The label column is a fixed width. A grid
  whose labels should all line up past their widest would make it a
  capped column of a `ColumnSchema`; the rows would then need to give way
  as one-line rows do instead of wrapping.
- **Inspecting a link.** A cut link label could report `Inspect` for its
  whole text; nothing asks for it yet, so only a cut property does.
- **Keys.** The grid stays keyless: the explorer that owns a grid's
  cursor carries the key table (`EXPLORER_KEYS`). A grid drawn without an
  explorer binds its host's keys.
- **Mouse hit testing.** `GridLines::lines` maps one to one onto the
  drawn lines before any wrap; a host maps a click through its wrapped
  rows to a line and then to a row index.
