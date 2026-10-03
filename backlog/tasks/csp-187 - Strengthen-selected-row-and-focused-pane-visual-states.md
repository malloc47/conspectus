---
id: CSP-187
title: Strengthen selected-row and focused-pane visual states
status: Done
assignee: []
created_date: '2026-05-20 03:28'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-165
  - CSP-166
ordinal: 413000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Visual slice landed in the earlier polish commit. Behavioral
  half landed now: `Msg::ScrollPreviewBy(i32)` replaces the
  old `ScrollPreviewDown` / `ScrollPreviewUp` variants. The
  runtime adds a `remap_for_focus` pass between `translate`
  and the reducer — when `App::focus()` is `Focus::Right`,
  `j`/`k` translate to `ScrollPreviewBy(±1)` and
  PageUp/PageDown to `ScrollPreviewBy(±viewport)`. Uppercase
  `J`/`K` continue to scroll the preview regardless of focus.
  The status-bar hint switches between `j/k move · Enter
  expand` (left focus) and `j/k scroll preview` (right focus)
  so the operator can see `Tab`'s effect immediately.
- Original scope: make the active attach target and focused pane visually
  unmistakable. Use a full-row selected style for the left tree,
  a distinct but low-noise focus treatment for the active pane
  border/title, and right-pane scroll hints that only appear
  when the preview can scroll. `Tab` should have both visible
  and behavioral effects: navigation keys apply to the focused
  pane, and the status bar names the active keymap.
- Tests: reducer tests for focus-specific key handling; Ratatui
  snapshots for left-focus, right-focus, selected agent row,
  selected mux-candidate row, and scrollable vs non-scrollable
  preview states.
- Blockers: `CSP-165`, `CSP-166` v1 slices.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-011`
