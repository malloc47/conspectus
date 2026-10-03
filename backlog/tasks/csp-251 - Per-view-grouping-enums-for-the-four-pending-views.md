---
id: CSP-251
title: Per-view grouping enums for the four pending views
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies: []
ordinal: 447000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: introduce `MuxGrouping`, `UnionGrouping`, `PrsGrouping`,
  and `ForksGrouping` alongside their CSP-163 row-tree builders.
  Wire a `Grouping` dispatch enum so `App` state and config can
  carry a single field that narrows to the active view's enum.
  Sessions enum unchanged but joins the dispatch.
- Tests: row-tree builder unit tests for each view's grouping
  values; dispatch-enum cycle-to-next tests covering wrap-around.
- Blockers: ADR 0031; lands alongside `CSP-163` for each view.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in `src/tui/mod.rs` — `SessionsGrouping`,
`MuxGrouping`, `UnionGrouping`, `PrsGrouping`, `ForksGrouping`,
and a `Grouping` dispatch enum. Each landed alongside the
matching CSP-163 row-tree builder with cycle-wrap unit tests.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-002`
