---
id: CSP-322
title: Left-pane follow sync (opt-in view switching)
status: To Do
assignee: []
created_date: '2026-06-01 03:32'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-321
ordinal: 427000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: implement `[tui.detail].left_pane_sync = "follow"`
  per `docs/tui-detail-mockup.md`. In `follow` mode, when
  `mirror` would keep the left pane's previous selection
  because the focused node has no row in the current view, the
  left pane switches to a view that *does* have the row and
  selects it. Backspace restores the previous view and the
  previous selection together (breadcrumb stack carries view
  state). Drill hops whose neighbor kind has no top-level view
  (e.g. `runtime_process`, `fork` when no fork view exists)
  fall back to `mirror` behavior. Add a `none` mode that
  leaves the left pane completely untouched during drilldown,
  and add the Controls overlay entry for flipping between
  `mirror`, `follow`, and `none` mid-session.
- Tests: reducer tests for view-switching across each
  `(focused-node-kind, current-view)` pair; tests for the
  fallback-to-mirror behavior on view-less node kinds; tests
  for Backspace restoring view + selection; Controls overlay
  mode-flip tests.
- Blockers: `CSP-321`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-036`
