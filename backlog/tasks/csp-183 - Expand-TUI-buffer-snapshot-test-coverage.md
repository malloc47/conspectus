---
id: CSP-183
title: Expand TUI buffer-snapshot test coverage
status: To Do
assignee: []
created_date: '2026-05-19 23:23'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-166
ordinal: 410000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `insta`-backed snapshot tests covering the
  sessions-view default render at 80×24, the
  ambiguous-mux-expanded variant, the mux view (once
  `CSP-163` mux builder lands), the PR view (once `CSP-163`
  prs builder lands), a narrow 60-col terminal (depends on
  `CSP-181`), the search overlay (depends on `/`-key wiring),
  the help overlay, the empty-graph frame, the
  `--no-live-preview` frame, and the
  selection-retention-after-refresh frame. Keep snapshots
  deterministic with fixed fixtures.
- Blockers: `CSP-166` v1 slice; individual snapshot variants
  depend on the corresponding feature stories.
- **slice landed**: expanded `src/tui/ui.rs` coverage around
  right-pane focus, selected-row styling when focus moves,
  contextual status text, compact path labels, compact mux
  preview headers, and bottom-cropped mux captures. The broader
  snapshot matrix remains open.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-006`
