---
id: CSP-322
title: Left-pane follow sync (opt-in view switching)
status: To Do
assignee: []
created_date: '2026-06-01 03:32'
updated_date: '2026-10-31 08:15'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the drill and back stack is the framework's `tuikit::explorer::Explorer` over the host's `ExplorerSource` (ADR 0188); the mirror stays host-side, run on the explorer's `Drill` outcome in `src/tui/app/explorer_nav.rs`, which is where `follow` and `none` go. Two keys moved: the left pane's `Backspace` now returns from a reveal and no longer pops the right pane's drill, and on the right pane `Esc` is Back as well as `Backspace` (ADR 0188, ADR 0192), so "Backspace restores the previous view and selection" must fit that split. The Controls overlay is a `tuikit::settings_panel::SettingsPanel` (ADR 0176) whose rows are listed in `src/tui/widgets/controls.rs`; a row applies as it changes.
<!-- SECTION:NOTES:END -->
