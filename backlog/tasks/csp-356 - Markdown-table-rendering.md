---
id: CSP-356
title: Markdown table rendering
status: Done
assignee: []
created_date: '2026-06-03 13:09'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 222000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Out of scope (deferred): in-cell Markdown styling,
  column-type-aware shrink priority, key/value transpose
  fallback (Codex's `table_key_value.rs` is the documented
  follow-on), alignment override for narrative columns.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented as a **pre-processor** (option b in the
original scope, but staged *before* tui-markdown rather than
after). Root cause confirmed by reading the
`tui-markdown 0.3.7` source: it doesn't enable pulldown-cmark's
`ENABLE_TABLES`, so pipes flow through as paragraph text
without ever reaching a table handler. A surface-area survey
of the operator's local corpus (40 Claude / 38 Codex / 19
OpenCode sessions) found 103 GFM pipe-tables; 100% wider
than 40 cols, ~85% wider than 80. Reviewed
Claude/Codex/OpenCode renderers — Codex (Apache-2.0) ships
the most coherent design (column-type classification,
iterative shrink, key/value transpose fallback) at ~2,700
LOC. ADR 0054 records the decision to adopt `comfy-table`
(Apache-2.0/MIT) as a cheaper-but-aligned alternative:
`ContentArrangement::Dynamic` + `set_width(content_width)`
delivers Codex-style iterative-shrink-to-fit without the
porting tax. `src/viewer/table.rs` owns the small
segmentation state machine that splits each Message /
CompactionSummary body into `BodySegment::Markdown(&str)`
and `BodySegment::Table(ParsedTable)`, exempts fenced code
blocks, parses GFM alignment markers (`:---:`), and renders
via comfy-table's `UTF8_NO_BORDERS` preset (light header
rule + dotted column separators; matches the viewer's
gutter aesthetic). Border glyphs route through
`theme.secondary_text` so colour overrides via `[tui.theme]`
work. Render cache key already includes `content_width`, so
terminal resizes invalidate correctly with no reducer
changes. Cell-Markdown styling (bold, code spans, links)
drops in v1 — same compromise Codex shipped for years; the
`custom_styling`-feature + `ansi-to-tui` upgrade path is
documented in ADR 0054. 17 tests added (13 table-module
unit tests + 1 widget-level proof + 2 snapshots at 80 and
40 cols + 1 inferred from compile time); 136 viewer tests
pass total; clippy + fmt + full nextest green.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEWER-NATIVE-017`
