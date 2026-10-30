---
id: CSP-783
title: Draw an action menu's cursor inactive under its own steps
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29 15:38'
updated_date: '2026-10-30 19:28'
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
- [x] #1 An action menu under a step of its own draws its cursor row with selection_inactive and keeps its marker
- [x] #2 The visible change is listed in the summary and checked with conspectus tui --snapshot at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `ActionMenu::draw_under_step` draws the menu with its cursor row in `selection_inactive`, marker kept (the menu's `View` gains `focused`, as the settings panel's has); `draw` and it share one paint; the `Overlay` impl overrides `render_under_step` with it. Checklist and Palette open no steps today, so they keep the default.
2. Tests: a unit test that a menu under a step draws its cursor row with `selection_inactive` and its marker, on its own and on a stack (and as it is under an overlay that is no step); an under-step golden. The host's worktree step cases already carry `Case::step`, so `Contained` accepts the restyle.
3. Docs: action_menu.md (the under-step look, theme token, rule 10, the unfocused-menu seam), overlay.md's dimming seam, settings_panel.md's seam note; ADR 0171 status note, design.md, CHANGELOG.
4. Verify with `conspectus tui --snapshot` of the worktree menu's branch prompt and dialogs at 100x30 and 40x10 against the CSP-781 frames.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
1. `ActionMenu::draw_under_step` draws the menu with its cursor row in `selection_inactive`, marker kept (the menu's `View` gains `focused`, as the settings panel's has); `draw` and it share one paint; the `Overlay` impl overrides `render_under_step` with it. Checklist and Palette open no steps today, so they keep the default.
2. Tests: a unit test that a menu under a step draws its cursor row with `selection_inactive` and its marker, on its own and on a stack (and as it is under an overlay that is no step); an under-step golden. The host's worktree step cases already carry `Case::step`, so `Contained` accepts the restyle.
3. Docs: action_menu.md (the under-step look, theme token, rule 10, the unfocused-menu seam), overlay.md's dimming seam, settings_panel.md's seam note; ADR 0171 status note, design.md, CHANGELOG.
4. Verify with `conspectus tui --snapshot` of the worktree menu's branch prompt and dialogs at 100x30 and 40x10 against the CSP-781 frames.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
An action menu under a step of its own now draws its cursor row in the theme's `selection_inactive`, its marker kept, as the settings panel does under its editors (house rule 10; commits abf52701, 4da2f9b3). `ActionMenu::draw_under_step` paints the menu with its `View` unfocused, `draw` and it share one paint, and the `Overlay` impl overrides `render_under_step` with it, so the host's row, mux, and worktree menus (`Box<ActionMenu<Msg>>` variants of its modal enum, which already dispatches `render_under_step`) need no change. Checklist and Palette open no steps today and keep the default. The action menu page (the under-step look, `selection_inactive`, rule 10, the unfocused-menu seam), the overlay page's dimming seam, and the settings panel page's seam note say so; ADR 0171's status, `docs/design.md`, and the changelog record it.

Verification: a unit test draws the menu under a step (directly and on a stack) with its cursor row in `selection_inactive` and its marker, and under an overlay that is no step of it in `selection_active`; an under-step golden keeps the colors. The host's worktree step cases carry `Case::step`, and the conformance suite passes with the restyle. `conspectus tui --snapshot` of the worktree menu's branch prompt (and its refused state) at 100x30, 60x20, and 40x10 against the CSP-781 frames: the menu's `► New worktree…` row drops from bold reversed to bold, marker kept; every other frame is unchanged (the dialogs cover the menu's cursor row). Frames in /tmp/tuifw-baseline/frames-783-after. fmt, clippy `-D warnings`, the isolated suite (4747 passed, 0 failed, 1 ignored), rustdoc `-D warnings`, `cargo build --examples`, and `git diff --check` pass.

**Regressions**
- None.

**Consistency wins**
- The worktree menu's cursor row recedes (bold, `►` kept, no reverse) while its branch prompt has the keys, at 100x30 and 40x10 alike, as Controls' row does under its editors: one look for a list whose step has the keys (house rule 10). Under the dialogs the menu's row is covered, so nothing there changes.
<!-- SECTION:FINAL_SUMMARY:END -->
