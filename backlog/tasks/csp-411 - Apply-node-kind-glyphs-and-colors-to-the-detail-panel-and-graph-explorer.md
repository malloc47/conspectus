---
id: CSP-411
title: Apply node-kind glyphs and colors to the detail panel and graph explorer
status: Done
assignee: []
created_date: '2026-06-09 14:32'
labels:
  - h-vis
milestone: m-17
dependencies: []
ordinal: 523000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`kind_chip_span` (`src/tui/ui.rs`) now emits the per-
kind slate glyph in the node-kind color rather than dim
`[kind]` text; `ForgePr` reuses `theme.pr_open` at chip
surfaces because that layer does not carry PR state. The
right-panel title (`right_panel_title`) prepends the kind
glyph before the bold kind label and folds the extra cells
into the breadcrumb-chain budget. `render_group_header_line`
in the relationship explorer swaps the prior textual
neighbor-kind column for the glyph, keeping the count anchor
via fixed padding. `NodeKind::from_snake_case` round-trips the
stable kind tag so call sites that carry the kind as
`&'static str` (field `kind_chip`, explorer `neighbor_kind`)
look up the slate without going through `GraphNode`. Showcase
fixture verified: agent-session detail pane reads
`● session` in the title, `◇` next to the cwd field, and
`◇`/`▦`/`▣` on relationship-explorer rows. Unit tests cover
the chip glyph + color per kind, the unknown-tag fallback,
the right-panel title prefix, and the explorer link-row
glyph ordering. Non-TUI surfaces stay under CSP-412.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIS-004`
