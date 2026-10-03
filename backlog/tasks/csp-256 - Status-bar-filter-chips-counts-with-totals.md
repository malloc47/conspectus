---
id: CSP-256
title: Status-bar filter chips + counts-with-totals
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies:
  - CSP-250
  - CSP-180
ordinal: 452000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: new status-bar zone left of provider chips, rendering
  active filter chips with ADR 0022 colors and stable ordering
  (harness → max-age → mux-state → future dimensions). Truncate
  long lists with `+N more`. Header counts switch to
  `<filtered> of <total>` form (`12 of 47 agents · …`).
- Tests: status-view unit tests for each chip layout; Ratatui
  snapshots for one-chip / many-chips / truncated / cleared
  states.
- Blockers: `CSP-250`, `CSP-180` (provider chip zone).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped — `render_filter_chips` renders the active
filter set with stable ordering (harness → max-age →
mux-state) and `format_count_with_filtered` shifts header
counts to `<filtered>/<total>` form when a filter narrows the
set. The chip zone landed alongside the CSP-418 header
rewrite rather than as the originally-scoped status-bar zone.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-007`
