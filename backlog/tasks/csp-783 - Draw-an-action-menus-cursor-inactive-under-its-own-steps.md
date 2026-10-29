---
id: CSP-783
title: Draw an action menu's cursor inactive under its own steps
status: To Do
assignee: []
created_date: '2026-10-29 15:38'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
type: enhancement
ordinal: 869000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The overlay stack tells an entry that a step of its own is open over it: `ModalStack::draw` calls `Overlay::render_under_step` for it (CSP-744). The settings panel overrides it, so the Controls panel's cursor row takes `selection_inactive` while one of its editors has the keys (house rule 10). `ActionMenu` does not override it, so the worktree menu keeps its active cursor under its branch prompt and dialogs (visible around the compact prompt, and beside a dialog on a narrow screen).

Scope: give `ActionMenu` (and any other list that can have a step, such as `Checklist` or `Palette` if one ever opens a step) a `draw_under_step` and a `render_under_step` override that draws its cursor row inactive, as `SettingsPanel::draw_under_step` does. Update the action menu's page, its seam note in `overlay.md`, and the CSP-744 seam note in `settings_panel.md`.

Tests: a unit test that a menu under a step draws its cursor row with `selection_inactive`; the host's worktree step cases in `src/tui/consistency_tests/conventions.rs` already carry `Case::step`, so the Contained check accepts the restyle.

Manual check: `conspectus tui --snapshot` of the worktree menu's branch prompt at 100x30 and 40x10; list the visible change (a consistency win under house rule 10).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An action menu under a step of its own draws its cursor row with selection_inactive and keeps its marker
- [ ] #2 The visible change is listed in the summary and checked with conspectus tui --snapshot at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
