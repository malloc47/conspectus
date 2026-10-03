---
id: CSP-195
title: Auto-scroll the left tree to keep the selected row visible
status: Done
assignee: []
created_date: '2026-05-20 13:38'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-166
  - CSP-183
ordinal: 442000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: today the left panel renders all visible rows into a
  single `Paragraph` with no viewport awareness, so once the
  selection moves past the rendered area the user can keep
  pressing `j` and see nothing change. Track a per-render
  scroll offset that follows the selection — at minimum,
  bring the selected row to the top edge when it moves
  above the viewport and to the bottom edge when it moves
  below. Page-down / page-up should jump a viewport at a
  time without losing the selection. Same-line previews must
  not affect viewport math.
- Tests: reducer + render unit tests for selection moving past
  the visible top/bottom in a small viewport, PageDown jumping
  by viewport height. Ratatui snapshots for a
  short and a long tree at the same viewport size.
- Blockers: `CSP-166` v1 slice. Friendlier after `CSP-183`
  expands the snapshot harness.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`App` gained a `Cell<u16>` left-panel scroll offset
and an `adjust_left_scroll(selected_line, viewport_height)`
method that nudges the offset only when the selected row
falls outside the viewport (above the top edge or at/below
the bottom edge). The renderer tracks which line index the
selected row lands at, asks `App::adjust_left_scroll` for
the offset, and passes it to `Paragraph::scroll`. Three
reducer-level tests plus a render-level test cover the
behavior. The render-level regression uses a long pre-selected
group row to prove left-tree rows clip rather than wrap, then
asserts the selected row lands on the bottom visible
left-panel line after `End`. PageUp/PageDown already drive the
viewport via
the existing reducer; this story just keeps the rendered
view in sync. Open: pixel-precise centering on first focus
and a manual-scroll keymap remain follow-ons under
`CSP-183`'s broader snapshot matrix.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-019`
