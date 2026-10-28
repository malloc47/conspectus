---
id: CSP-757
title: 'Extract `AppFrame`, `SplitPane`, and `PaneFrame` with the focus cue'
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-chrome
milestone: m-25
dependencies:
  - CSP-726
  - CSP-721
  - CSP-729
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
  - docs/adr/0076-scrollbar-widget-choice.md
priority: medium
type: enhancement
ordinal: 843000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`ui::draw` is a screen shell that takes `&mut App` and dispatches viewport messages so the reducer can reconcile scroll offsets. The body split is copied three times (`ui.rs`, `snapshot.rs`, and test fixtures). Focus is shown only by a `► ` in the pane title: borders never change, whereas modal borders always use the accent, and ADR 0076 says the focused pane's border is highlighted. `runtime.rs:843` is a three-way switch above the chrome (inventory slice 07, section 1).

Scope: Create `AppFrame` (header row, body, footer row, overlay layer, toast layer), `SplitPane` (side by side above a width threshold, stacked below), and `PaneFrame` with a focus marker and an accent-tinted border. The snapshot code and test fixtures use the framework's split. The renderer returns measured sizes instead of dispatching messages. The header composition and which view fills the body stay in Conspectus.

TUI changes:
- Consistency wins: the focused pane is visible at a glance; one split implementation; the pane and modal frames share one vocabulary
- Regressions or behavior changes: the focused pane's border and title marker take the accent colour; the unfocused pane keeps the plain border

Tests and checks: `--snapshot` of the main screen with each pane focused, at wide and stacked widths. The snapshot driver and tests use the same split.

Blockers: The modal frame and the overlay contract.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `AppFrame`, `SplitPane`, and `PaneFrame` are in the framework and the body split exists once
- [ ] #2 The focused pane's border and title marker use the accent colour, matching ADR 0076
- [ ] #3 `ui::draw` no longer dispatches messages
- [ ] #4 `snapshot.rs` and test fixtures use the framework's split
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
