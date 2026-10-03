---
id: CSP-409
title: Define the per-node-type glyph and color assignments
status: Done
assignee: []
created_date: '2026-06-09 14:32'
labels:
  - h-vis
milestone: m-17
dependencies: []
ordinal: 521000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/tui/icons.rs` ships the `NodeKind` enum with
`From<&GraphNode>` / `From<&NodeId>` conversions, the
`NodeKindStyle { glyph, color, width }` struct, and the
`node_kind_style(kind, theme)` lookup. `Theme` gains eight
`node_*` color fields (defaults from ADR 0073) plus an
`icons: IconOverrides` field for `[tui.theme.icons]` operator
overrides validated to 1-cell width by `parse_icon_override`.
`ForgePr` returns the documented `Color::Reset` sentinel — PR
rows pick from `theme.pr_*` based on state. Unit tests cover
the slate catalog, default-glyph widths, `NodeId` conversion,
operator overrides, and the config-loader path
(`[tui.theme.icons]` parsing + diagnostics for unknown keys,
non-string values, wide glyphs). `CSP-410` consumes these
primitives to apply the glyph prefix to row rendering.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIS-002`
