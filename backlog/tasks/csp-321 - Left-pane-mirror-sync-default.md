---
id: CSP-321
title: Left-pane mirror sync (default)
status: Done
assignee: []
created_date: '2026-06-01 03:32'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-314
ordinal: 426000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: implement `[tui.detail].left_pane_sync = "mirror"` as
  the default behavior per `docs/tui-detail-mockup.md`'s
  Left / Right Pane Synchronization section. When the right pane
  drills via `Enter` on a relationship row, the left tree
  scrolls to and selects the row corresponding to the focused
  node, expanding group rows along the ancestor path. The
  left-pane *view* does not change. When the focused node has
  no row in the current view (e.g. `runtime_process` while in
  sessions view), the left pane keeps its previous selection.
  The breadcrumb stack also stacks left-pane selection state so
  `Backspace` restores both panes. Manual left-tree navigation
  cancels the active drill: the right pane's focused node is
  replaced by the node corresponding to the new tree selection
  and the breadcrumb stack is collapsed.
- Tests: pure tree-expansion tests for finding/selecting a
  node by id across each left-pane view; reducer tests for
  drill + sync, Backspace restoring prior selection, manual
  tree navigation collapsing the drill, and the "focused node
  has no row" fallback. Buffer snapshots for at least the
  sessions and mux views across one round of drilldown.
- Blockers: `CSP-314`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-035`
