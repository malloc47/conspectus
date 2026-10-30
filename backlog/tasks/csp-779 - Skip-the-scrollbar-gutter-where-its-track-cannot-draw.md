---
id: CSP-779
title: Skip the scrollbar gutter where its track cannot draw
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29 09:42'
updated_date: '2026-10-30 19:57'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 865000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`tuikit::scroll::ScrollGutter::layout` carves a 1-column gutter whenever the rows overflow, but `ScrollGutter::render` keeps a row clear above and below the track and ratatui draws nothing once the track has no room between its arrows, so a scrolled body shorter than five rows keeps a blank column and shows no bar. The action menu at 40x10 is the visible case (its entries get four rows, so labels lose a column for nothing); the help page, the panes, and the preview can hit it on very short terminals.

Scope: make the gutter fade when its track cannot draw (no gutter below the height that leaves at least one track cell), in `ScrollGutter::layout`, so every scrolled body gets the column back. Update the scroll page and the action menu page's seam.

Tests and checks: gutter tests at heights 3 to 6; the action menu's goldens and frames at 40x10; `--snapshot` of the help page and a pane at a very short height.

Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `ScrollGutter` gains `MIN_HEIGHT` (5: the track's two margin rows, the two arrows, one cell of track) and `carves(width, height, rows)`, the one rule; `layout` carves only when it holds.
2. Every place that predicts the gutter before drawing asks the same rule: `WrappedRows::fit`, `ModalFooter::fit_beside`, the pager's footer fit, and the log list's footer beside its detail; the palette fits its footer under its query rows so the rule sees the list's own height.
3. Tests: gutter tests at heights 3 to 6 (carved or not, a bar for every gutter carved), `fit` and `fit_beside` below the height, the tree's producer width in a 4-row pane, and an action menu at 40x10 whose labels take the last column; the 40x10 golden is checked.
4. Docs: scroll page (the gutter, the API table, wrapped rows), the action menu page's paragraph and seam, the checklist seam, the frame page's `fit_beside` row; ADR 0156 status note, ADR 0171's consequence, design.md, CHANGELOG.
5. Verify with `conspectus tui --snapshot`: the row menu at 40x10, the help page and the panes at a very short height, before and after.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented `MIN_HEIGHT` and `carves` in `scroll/gutter.rs`; `WrappedRows::fit`, `fit_beside`, the pager's `fit`, and the log list's `fit_footer` use it. The palette's footer fit moved under its query rows (identical rows, exact list height for the rule). Snapshot comparison: 63 of 295 frames differ, all with an overflowing body under five rows; the Search view's pane row at 80x7 now matches a tall 80-column frame. Full suite 4750 passed, 0 failed, 1 ignored.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A scrolled body too short for the scrollbar's track now keeps its last column (commits 9f6d238b, 8d9415a8). `ScrollGutter::MIN_HEIGHT` (5: the two margin rows, ratatui's two arrows, one track cell) and `ScrollGutter::carves(width, height, rows)` are the one rule: `layout` carves only when it holds, and `WrappedRows::fit`, `ModalFooter::fit_beside`, the pager's footer fit, and the log list's footer beside its detail ask it, so nothing is laid out a cell narrower for a bar that cannot draw. The palette now fits its footer under its query rows, so the rule sees the list's own height (same rows as before at every size). A palette that reserves the column (the searches) still reserves it. Docs: the scroll page (the gutter, the API table, wrapped rows), the action menu page (paragraph; its short-screen seam removed, and the checklist page's copy), the frame page's `fit_beside` row; ADR 0156 status (decision 5 refined), ADR 0171's consequence, design.md, CHANGELOG, operator review.

Verification: gutter tests at heights 3 to 6 (carved from 5, a thumb in every gutter carved, a blank column never kept), `fit` and `fit_beside` below the height, the tree's producer width in a 4-row pane, and an action menu at 40x10 whose 36-cell labels show whole; the 40x10 action menu golden is unchanged (its labels are short). `conspectus tui --snapshot` before/after of every framed modal at 100x30, 40x10, 100x8, 60x7, every view at 160x40, 60x20, 160x8, 80x7, 40x10 (top, end, middle, right pane focused), and the help page at 50 to 120 columns by 7 to 9 rows: 63 of 295 frames differ, all at heights where a list overflows under five rows; none at 100x30, 160x40, or 60x20 except one invisible trailing-cell style in the Pinned view's Preview at 60x20. Frames in /tmp/tuifw-baseline/frames-779-{before,after}. fmt, clippy `-D warnings`, the isolated suite (4750 passed, 0 failed, 1 ignored), rustdoc `-D warnings`, `cargo build --examples`, and `git diff --check` pass.

**Regressions**
- At 80x7 the Search view's pane row drops its `zsh` command: laid out a cell wider, its name and snippet take the widths a tall 80-column frame gives them, and the command, first to give way, leaves, as it does in the tall frame.

**Consistency wins**
- Menus at 40x10 and 60x7 (row menu, worktree menu, Clean up, Close down) and the rename prompts' and value modal's bodies take back the blank column: labels and details cut one cell later (`Forget · press Enter again to co…`, `remove the record from history` on one row).
- Short panes (Sessions, Mux, PRs, Pinned, Search at 160x8, 80x7, 40x10) lay their rows out at the width a tall pane has, so stepped columns (the mux name), previews, and directories read as they do on a tall terminal (`project-main   live       5m`, `Showcase: codex resume via…`).
- The help page under five body rows wraps at its full width (at 70x8 `Space selects or` stays on one row).
<!-- SECTION:FINAL_SUMMARY:END -->
