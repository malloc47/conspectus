---
id: CSP-502
title: Make the column-reflow (narrow → stacked) threshold configurable
status: Done
assignee: []
created_date: '2026-07-28 03:30'
labels:
  - h-layout
milestone: m-18
dependencies: []
ordinal: 527000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `NARROW_LAYOUT_THRESHOLD` (`src/tui/ui.rs:56`, hard-coded
  `100`) governs when `draw_body` switches the side-by-side left/right
  panes to a vertical stack; `src/tui/snapshot.rs` reads the same
  constant. Add a `[tui] narrow_layout_threshold` (columns) config key
  with the current `100` as the default, thread it through `App` state
  the way `theme` / `show_harness_chips` already flow, and have both
  `draw_body` and the snapshot path read the resolved value.
- Tests: config merge test for the new key (default, override,
  malformed → diagnostic); a `draw_body`/snapshot test proving the
  split direction flips at the configured width.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): landed in `93406b5` as `[tui]
narrow_layout_threshold`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-LAYOUT-001`
