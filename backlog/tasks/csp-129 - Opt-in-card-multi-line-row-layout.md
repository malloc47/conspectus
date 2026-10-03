---
id: CSP-129
title: Opt-in card / multi-line row layout
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 121000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`Layout::Card` joins `Layout::Columnar` in `RenderOptions`,
with `RenderOptions::card()` and `RenderOptions::card_width(n)`
convenience constructors. The new `render_card` path emits one
`KEY: value` line per column with keys aligned on the colon and a
blank line between rows. Width-aware mode truncates long values
(using the same `truncate_to_width` helper as columnar) so a
`--width N` budget is honored. The `session` subcommand gained a
`--layout {columnar|card}` flag (default columnar). Added four unit
tests covering empty snapshots, block separation, colon alignment,
and width-aware truncation, plus a CLI integration test for
`--layout card`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-004`
