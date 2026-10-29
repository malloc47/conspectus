# `richtext`

| | |
| --- | --- |
| Module | `tuikit::richtext` |
| Layer | 1 (foundation) |
| Type parameters | none |
| Cargo feature | `richtext`, off by default; it brings `ansi-to-tui`, `tui-markdown` (with the syntax highlighter its fenced-code highlighting uses), and `comfy-table` |
| Outcomes | none: every function returns styled lines, and nothing here handles a key |

## Purpose

`richtext` turns rich content into styled lines laid out at a width the
caller gives, so any body draws it as it draws its other rows: a pager's
content, a preview, a detail pane. Programs that show such content grow
the pieces one viewer at a time: an ANSI parsing shim with its own
fallback for color being off, a layout for a terminal's captured screen
that keeps its rules and boxes from wrapping into rubble, a Markdown
renderer with its own fixes (a loose list's marker on a line of its own),
a pre-pass that finds pipe tables because the renderer does not, and a
table layout whose borders need to recede. The module owns them once.

It is a feature because the parsing crates are heavy (the highlighter
carries its grammars and themes); a program that shows no rich content
leaves it off and builds none of them.

The host keeps where the text comes from (a capture it ran, a document it
read), which wrap mode a person picked and how that setting is spelled in
its configuration, the width it lays out at, and how the lines are framed:
a gutter beside them, a window onto them ([`ScrollRegion`](scroll.md#the-region)
or a [pager](pager.md)), a find painted over them.

## Usage example

A terminal capture fitted to a narrow area, and a Markdown body with a
table, at the same width.

```rust
use ratatui::text::Line;
use tuikit::richtext::{WrapMode, ansi_text, layout_capture, markdown};
use tuikit::theme::Theme;

// A capture: parse its escapes (or drop its styles when drawing without
// color), then fit it to 30 cells in the default mode.
let raw = "\x1b[32mok\x1b[0m all checks passed\n\u{2500}\u{2500}\u{2500}\n\n";
let parsed = ansi_text(raw, true);
let rows = layout_capture(&parsed.lines, WrapMode::Smart, None, 30);
assert_eq!(rows.len(), 2, "the trailing blank row is dropped");
assert_eq!(rows[0].spans[0].content, "ok");

// A body of Markdown: prose, a list, and a table, wrapped to 30 cells.
let body = "Some *emphasis*.\n\n- one\n- two\n\n| key | value |\n|---|---|\n| a | b |";
let lines: Vec<Line> = markdown(body, 30, &Theme::default());
assert!(lines.iter().all(|line| line.width() <= 30));
```

### The functions

| Item | Does | Notes |
| --- | --- | --- |
| `ansi_text(raw, color)` | text with ANSI escapes as styled text: each SGR sequence styles the spans after it | with `color` off, the characters with no styles; text the parser refuses comes back as it is rather than not at all |
| `layout_capture(lines, mode, source_width, width)` | a parsed capture laid out as rows no wider than `width` | trailing blank lines (only whitespace, whatever their style) go first, so the newest output sits at the bottom; tabs stop every 8 columns |
| `WrapMode::{Plain, Smart, None}`, `WrapMode::ALL` | how a capture's lines fit an area narrower than their source | `Smart` is the default; `ALL` lists the default first |
| `markdown(body, width, theme)` | Markdown as lines no wider than `width` | headings, emphasis, lists, quotes, links, and inline code styled; fenced code highlighted by its info string; pipe tables laid out by `render_table`; blank lines between blocks kept |
| `code(source, language)` | source in `language` (an info string: `rust`, `py`, `toml`) as highlighted lines | one line per source line, not wrapped; a language the highlighter does not know gives plain lines |
| `segment_body(body)` | a body of Markdown as `BodySegment`s: Markdown slices and `ParsedTable`s | fenced code is never a table; a row of the wrong width is padded or cut to the header's |
| `render_table(table, width, theme)` | a `ParsedTable` laid out `width` cells wide | columns sized to their content, cells wrapped, alignments from the `:---:` markers |

### The wrap modes

A capture is laid out for the width of the screen it came from, which is
usually wider than the area showing it.

- **Plain** wraps every line at the area's width by the text module's
  [wrapping rules](text.md#wrapping-rules), rules and borders included.
- **Smart** wraps content but not formatting. Per line it drops the
  trailing padding, then: cuts the line at the edge when everything past
  it is decoration (box drawing, block elements, dashes and rules) or
  blank; otherwise squeezes interior runs of two or more spaces, widest
  first, when that fits the line on one row; otherwise wraps it with a
  hanging indent as deep as its leading indent (when that indent is less
  than half the width).
- **None** keeps the source's own layout: each line re-wrapped by
  character at `source_width`, the width the source laid it out at, then
  each row clipped to the area with no mark, as a terminal clips. Without
  a source width it clips each line.

Every row fits the width in every mode.

### Markdown and tables

The Markdown renderer does not parse GFM pipe tables, so `markdown` first
splits the body with `segment_body`: the Markdown between tables goes
through the renderer and each table through `render_table`. A loose list
(items separated by blank lines) has its markers put back on the line of
their item's text. Each rendered line is wrapped to the width.

A table is laid out with a double rule under its header and dotted lines
between columns, no outer box (which would compete with whatever frames
the text beside it), each column as wide as its content allows, and the
cells wrapped to fit. The border characters take the theme's inert tone;
the cells are plain text. In-cell Markdown (bold, code spans, links) is
dropped, and a table whose narrowest columns still overflow keeps the
layout's overflowing output.

## Key table

None: the module handles no keys. The body that shows its lines scrolls
them.

## Theme tokens

| Token | Used for |
| --- | --- |
| `inert` with `muted` | a table's borders (`Tone::Inert`) |

Everything else keeps the styles the content brings: ANSI text its own
colors, Markdown the renderer's style sheet (bold headings, italic
emphasis, the list numbers' color), and code the highlighter's theme.
Mapping those onto theme roles is a seam below.

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 13 | Theme | the one style the module chooses, a table's borders, reads the theme's inert tone; the rest is the content's own (see the seam below) |
| 14 | Width | every line is laid out in terminal cells: wrapped by the text module's rules, a wide character counted as two cells, a capture's rows fitted to the width in every mode |

Not applicable: 1 to 12 and 15 to 18 govern how controls look, move,
confirm, close, label, and edit. The module only lays text out; the body
that shows it follows those rules.

## Seams left open

- **Themed Markdown and code.** The renderer takes a style sheet and the
  highlighter a theme; building both from theme roles (heading, emphasis,
  code, link) would let a host's palette reach them.
- **Styled cells.** Cells could keep their Markdown styling by rendering
  each through the Markdown renderer and the table through ANSI text.
- **Narrow tables.** A table that cannot fit could fall back to key and
  value records instead of overflowing.
- **One escape stripper.** `text::strip_ansi` drops escapes for plain
  comparison; `ansi_text` with color off keeps the parser's reading of
  them. A reader that wants plain text from a capture could share one.
- **Source widths.** A capture's source width is the host's to report; a
  capture without one clips in the no-wrap mode instead of re-wrapping.
