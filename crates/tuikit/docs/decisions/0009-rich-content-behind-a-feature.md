# 0009: Rich content sits behind an optional feature

| | |
| --- | --- |
| Status | Accepted |

## Decision

- The `richtext` cargo feature, off by default, turns on the `richtext`
  module and its three optional crates: `ansi-to-tui` (ANSI text into styled
  lines, without its default features), `tui-markdown` with its code
  highlighting (which brings `syntect`), and `comfy-table` (table layout to a
  width with dynamic arrangement).
- The module holds ANSI text with a path for drawing without color and a
  fallback for malformed input; the layout of a captured screen in three wrap
  modes (record [0058](0058-captured-screen-wrap-modes.md)); Markdown at a
  width with GFM pipe tables, found by a pre-pass that splits the body into
  Markdown and table segments before the Markdown renderer sees it, and
  fenced code highlighted by its language; and an entry point that highlights
  code outside Markdown.
- The glyphs a rendered table's borders draw are in the framework's vetted
  table, and the set that tells a border from a cell is a constant of the
  glyphs module.

## Rationale

Only rich content needs these crates, and they are heavy, the highlighter
most of all; a host that shows none builds none of them. Each does one job
the framework would otherwise own with a long tail of edge cases: an ANSI
parser (colors in three depths, operating-system commands), a Markdown
renderer (lists, breaks, nested emphasis), and the iterative shrink-to-fit
width allocation that table layout needs. The Markdown renderer does not
parse tables, so the pre-pass finds them, skipping fenced code.

## Consequences

- `cargo test -p tuikit` runs without the feature and with it, and nothing
  outside the module may come to need it (the checks build both ways).
- Table cells are plain text: in-cell emphasis, code, and links are dropped.
- The module's page is [controls/richtext.md](../controls/richtext.md).

## Alternatives considered

- **Rich text always on.** Simpler for a host that wants it, but every other
  host pays for the highlighter.
- **No rich text in the framework.** The pager would have no rich content.
- **A hand-written ANSI parser**, **a Markdown renderer over a bare
  CommonMark parser**, or **a larger ported table renderer** with a key and
  value fallback for very narrow widths. Each is more code to own for a
  marginal gain; the last stays a possible follow-up.
- **A Markdown crate that draws to the terminal itself** (`termimad`). It
  owns its drawing loop and produces no ratatui text.
- **Telling borders from cells by Unicode block.** Shorter than a set, but a
  box-drawing character inside a cell would take the border's style.
- **No entry point for code outside Markdown.** Callers reached highlighting
  only through fenced code; the entry point serves content that shows a
  source file.

## Origin

- host ADR 0152, "Who owns the framework's third-party crates": decision 3
  (the rich-text crates behind a feature).
- host ADR 0181, "Rich text is a framework feature, and the preview scrolls
  in a region": decisions 1, 2, and 4.
- host ADR 0025, "An ANSI rendering dependency for previews": adopting
  `ansi-to-tui` without its default features, and the color-off path.
- host ADR 0051, "A Markdown rendering dependency": adopting `tui-markdown`,
  and later turning its code highlighting on.
- host ADR 0054, "Markdown table rendering through comfy-table": the table
  layout and the segmenting pre-pass.
