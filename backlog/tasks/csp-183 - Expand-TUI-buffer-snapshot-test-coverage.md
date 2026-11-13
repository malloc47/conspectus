---
id: CSP-183
title: Expand TUI buffer-snapshot test coverage
status: To Do
assignee: []
created_date: '2026-05-19 23:23'
updated_date: '2026-10-31 08:15'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): `tuikit::testkit` (ADR 0164) now offers the key-script DSL, a one-frame driver, style-keeping goldens (`testkit::golden`, accepted through `TUIKIT_ACCEPT_GOLDENS`), and the conformance suite that holds every registered overlay to the shared conventions; `src/tui/consistency_tests/` covers the host's views. The search overlay is now the quick-select and content-search palettes, and the help overlay the generated `?` page (ADR 0192), both already in the conformance registry (`src/tui/consistency_tests/conventions.rs`). What remains is the view-level frame matrix: the views at 80x24 and at 60 columns, ambiguous mux expanded, the empty graph, `--no-live-preview`, and the selection kept across a refresh.
<!-- SECTION:NOTES:END -->
