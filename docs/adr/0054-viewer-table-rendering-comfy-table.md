# ADR 0054: Markdown Table Rendering Via comfy-table

## Status

Accepted. Extends ADR 0052 (native viewer) and ADR 0051 (markdown
rendering dep). Adds `comfy-table` to the viewer's allowed-dep
surface.

## Context

`H-VIEWER-NATIVE-017` (Markdown table rendering) needs to fix a
real readability gap: tables in agent transcripts arrive as raw
GFM pipe-tables, and tui-markdown 0.3.7 doesn't enable
`pulldown_cmark::Options::ENABLE_TABLES` in its parser. Pipes flow
through as paragraph text, so the viewer shows literal `|` rows.

Survey of the operator's corpus (40 Claude sessions, 38 Codex, 19
OpenCode):

- 35 / 97 sessions (36%) contain at least one Markdown table.
- 103 table blocks total. Median width 132 cols; 100% wider than
  40 cols; ~85% wider than 80 cols.
- Tables come exclusively from assistant turns. Cell content
  routinely carries Markdown formatting (bold, code spans, links)
  and the wider tables carry multi-sentence paragraphs.
- All three harnesses store the same shape: raw GFM pipe-tables
  emitted by the LLM. No harness pre-renders ASCII anywhere on
  disk. Rendering happens in the harness's UI at chat time.

Harness rendering reference (per the [feedback-viewer-match-harness]
principle from ADR 0053):

- **Claude Code** uses Ink (React-for-CLI) with `ink-table`-style
  rendering. Aligned ASCII tables at fit-widths; documented
  breakage at extremes (#22311 "tables disappear at full-screen",
  #11274 CJK misalignment, #37808 link-text expansion). Closed
  source; no documented overflow fallback.
- **Codex (Rust CLI)** ships a custom ~2,700-LOC renderer in
  `codex-rs/tui/src/markdown_render.rs` (Apache-2.0):
  column-type classification (narrative / token-heavy / compact),
  iterative width shrinking with per-column floors, and a
  **key/value transpose fallback** (`table_key_value::render_records`)
  when even minimum widths can't fit. ━ header sep, ─ row sep,
  2-cell column gap, 1-cell padding. Recently overhauled (v0.131
  responsive tables, v0.136 cramped-table key/value records).
- **OpenCode** uses Glamour in the released path (still shows raw
  pipes — issues #3845, #7671); a newer opentui `<markdown>`
  component is gated behind `OPENCODE_EXPERIMENTAL_MARKDOWN` and
  has known border/alignment bugs (#12164, #1272).

All three harnesses are converging on the same target — aligned
text tables with content-aware width allocation and a fallback
for too-narrow widths. Codex is the only one that has fully
shipped the fallback. That makes Codex the de facto reference
implementation when "match harness UX" needs a single answer.

## Decision

Render Markdown tables in the viewer via the `comfy-table` crate
(Apache-2.0 / MIT), invoked as a pre-pass that splits each
message body into Markdown segments and table segments before
tui-markdown sees the body. Table segments render through
`comfy-table` with `ContentArrangement::Dynamic` and
`set_width(content_width)`; Markdown segments continue through
tui-markdown unchanged.

### Why comfy-table over the alternatives

`ContentArrangement::Dynamic` is exactly the iterative-shrink-to-
fit-width algorithm Codex hand-implements. comfy-table also
handles per-cell wrapping, alignment markers (`:---:`), and
Unicode width measurement out of the box. The two things it
doesn't do that Codex does:

1. Column-type classification (narrative / token-heavy / compact)
   with priority-based shrinking.
2. Key/value transpose when even minimum widths don't fit.

Both are quality improvements for the tail of the distribution.
Neither is required for v1 to make the viewer's existing output
dramatically more readable. The 90th-percentile case in our corpus
is "wide table, reasonably wide terminal, just needs cell
wrapping" — and that's exactly what comfy-table covers.

Considered and rejected:

- **Status quo**: 85% of observed tables are unreadable.
- **Pre-process tables into fenced code blocks**: cheap, but
  doesn't move the readability needle — 200-char rows still
  overflow inside the code block.
- **Hand-built renderer** (~500-700 LOC + tests): we'd own all of
  it. Defensible long-term but heavier than comfy-table for what
  is at this point a tactical readability fix.
- **Port Codex's renderer** (~4,000 LOC + transitive deps): tempting
  because it's the exact right algorithm with the key/value
  fallback. Defer. If operator feedback flags
  comfy-table's wrap-without-transpose behavior as the
  bottleneck, lifting Codex's `table_key_value.rs` as an
  additional fallback for `compute_column_widths == None` is the
  natural follow-on.
- **Fork or PR-upstream tui-markdown table support**: couples our
  release cycle to an upstream we don't control. A local
  pre-pass keeps the boundary clean and avoids touching the
  tui-markdown surface we already depend on.

### Cell-Markdown styling deferred

Cells in v1 render as plain wrapped text. In-cell bold, code
spans, and links are dropped — comfy-table accepts strings, not
styled spans. This is the same compromise Codex shipped for
years. If operator feedback flags it as a real loss, the upgrade
path is comfy-table's `custom_styling` feature: pre-render each
cell through `tui-markdown` → flatten to ANSI escape codes →
hand the encoded string to comfy-table → run the resulting
table string through `ansi-to-tui` to get back to Ratatui
`Line`s. That's an extension of this ADR, not a rewrite.

### Pipeline shape

In `src/viewer/render.rs` (or a sibling `table.rs`), introduce a
segmentation function:

```rust
enum BodySegment<'a> {
    Markdown(&'a str),  // borrowed slice from the body
    Table(ParsedTable), // header, alignments, rows
}

fn segment_body(body: &str) -> Vec<BodySegment<'_>>;
```

`build_body_lines` for `Message` / `CompactionSummary` turns
calls `segment_body`, then for each segment either feeds the
markdown to `tui_markdown::from_str` (existing path) or renders
the table via comfy-table + converts the rendered string to a
`Vec<Line<'static>>`.

Table detection is a small state machine over the source lines —
not a full Markdown parser — looking for the GFM shape:

- A header row: `|...|`.
- A separator row immediately after: `|---|---|...|` with optional
  `:` for alignment.
- One or more body rows: `|...|`.

This matches what pulldown-cmark recognizes when ENABLE_TABLES is
on, but doesn't require us to run pulldown-cmark twice. Lines that
look table-shaped inside fenced code blocks (` ``` `) are
exempted — code blocks are detected by the same state machine.

### Border style

Use `comfy_table::presets::UTF8_HORIZONTAL_ONLY` or
`UTF8_BORDERS_ONLY` (final choice during implementation, after
visual check). Border characters are restyled to
`theme.secondary_text` (matching the gutter rule's dim color) so
tables read as structurally distinct without competing with
turn-level chrome.

## Consequences

- `comfy-table` joins `ALLOWED_EXTERNAL_DEPS` and
  `docs/transcript-viewer-deps.md`. Per the dep-surface test in
  `src/viewer/mod.rs`, the doc + const + Cargo.toml must all
  agree before the change merges.
- The viewer module gains one renderer-side dep (`comfy-table`)
  and one downstream transitive (`crossterm` is already there;
  `unicode-width` is already there). No new heavy deps.
- The render cache key already includes `content_width`, so
  resizing the viewer correctly invalidates the table layout. No
  reducer changes needed.
- Snapshots for any widget test that renders a Message turn
  containing a table will need re-baselining once. Existing
  test fixtures don't contain tables so no churn beyond new
  tests.
- The Codex `<turn_aborted>` channel-marker filter and the
  aborted-message filter from ADR 0053 don't interact with table
  detection; tables are body-level content.

## Alternatives Considered

See "Why comfy-table over the alternatives" above.

## Open Questions

- **Key/value transpose for desperately narrow widths.** comfy-table
  will keep shrinking columns until they hit its 1-char floor;
  beyond that the table overflows the area. For our 10-cell gutter
  + typical terminal widths this is rare. Re-evaluate after
  operator feedback; the Codex `table_key_value.rs` port is the
  documented upgrade path.
- **In-cell Markdown styling.** Deferred per the rationale above.
  Same upgrade path documented.
- **Alignment column choice for narrative content.** comfy-table
  defaults left-align; the GFM `:---:` markers are honored.
  Whether to override default alignment when a column is
  classified as "narrative" (Codex's heuristic) is a follow-on
  improvement.
