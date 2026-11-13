---
id: CSP-786
title: Keep the left pane's row width when its list overflows
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 05:19'
updated_date: '2026-10-30 20:25'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 872000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`TreeView` carves the scrollbar gutter out of the pane only while the rows overflow it, so every row is laid out one cell narrower once the list grows past the pane. Columns whose width steps with the room or the row (the mux name, the Search view's hit tag) or takes a share of it can then jump by several cells: at 160 columns the Search view's rows are 78 cells while they fit and 77 once they overflow, which takes a hit's tag from 20 cells to 14 (`turn 46 · mes…`). The search palettes avoid this by reserving the scrollbar column (`Palette::reserve_scrollbar`, ADR 0175 decision 4). Decide whether the tree reserves the column too (rows always end a cell before the border) or lays its stepped and shared columns out from the pane's whole inner width, and record the choice.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A row of the left pane is laid out at one width whether or not its list overflows, or the chosen rule is recorded with the reason
- [x] #2 The Search view's hit rows at 160 columns keep one tag width as results stream past the pane
- [x] #3 Before/after --snapshot frames of every view at 160, 110, 80, 60, and 40 columns show only the intended changes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Decide: the tree keeps the pane's last cell while its rows fit and, when they overflow, lays them out at the pane's whole inner width with the fill lending the bar its cell (not a reserved column, which would cost every fitting row a cell and keep the 14-cell tag at 160 columns for good). Record it as ADR 0195.
2. `tuikit::columns::Room`: the cells the width rules count and the cells the row fits in (`Room::new(w).beside(1)`); `layout`, `draw`, `draw_split`, `draw_then` take `impl Into<Room>` (a `usize` is a room of that many cells). Steps 1 and 2 of the policy count the whole, steps 3 to 5 fit the cells.
3. `RowCtx::scrollbar` (1 while the gutter is carved) set by `TreeView`; `TreeColumns` lays rows out in `Room::new(width + scrollbar).beside(scrollbar)`.
4. Tests: `Room` conversions; a row beside a bar keeps its steps, row steps, share, and `drop_below` and the fill gives the cell; a fill at its minimum gives way by the policy; a row with no fill narrows by the policy; property tests (never wider than its cells; lined-up widths kept while the fill has a cell); a tree row laid out alike in a list that fits and one that overflows.
5. Docs: columns and tree view pages, docs/tui-columns.md prose (frames unchanged, checked by its test), ADR 0182/0183 status, design.md, CHANGELOG, operator review.
6. Verify with before/after `--snapshot` frames of every view and the Search view's shapes at 160, 110, 80, 60, 40 columns, tall and overflowing.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Chose the whole-width layout over a reserved column (ADR 0195): fitting frames are unchanged and an overflowing list reads as a fitting one. Frames: 652 captured before/after (every view top/end/middle/right-focused at 160, 110, 80, 60, 40 columns by 40 and 14 rows plus overflowing heights 9/10 and 16/18, the Search view's ten shapes at tall and short heights, and its message hits at 160 by 12 to 40 rows); 5 differ, all intended (the Search view's message hits at 160x12, 16, 20, 24 and at 80x40). docs/tui-columns.md frames are unchanged (its test passes); its prose gained the overflow rule. Full suite 4755 passed, 0 failed, 1 ignored.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A left-pane row is laid out at one width whether or not its list overflows (commits 564bf2dc, f53d88f4). The chosen rule, recorded in ADR 0195: the tree does not reserve the scrollbar's column (its rows keep the pane's last cell while they fit, as they always have); when the rows overflow, every row's width rules count the pane's whole inner width and the fill (preview, snippet, title, content, directory) gives the bar its cell, or, where it is at its minimum, the next column by the narrow-width policy. A reserved column was rejected: every fitting row would lose a cell, the mux name's steps would move again, and the Search view's tag at 160 columns would keep its 14-cell step for good. Framework: `tuikit::columns::Room` (`Room::new(w).beside(1)`: the rules count `whole`, the cells fit `cells`; a `usize` converts), taken by `ColumnSchema::layout`, `draw`, `draw_split`, `draw_then`; `RowCtx::scrollbar`, which `TreeView` sets while the gutter is carved and `TreeColumns` counts. Docs: the columns and tree view pages, the prose of docs/tui-columns.md, ADRs 0182 and 0183 (status), ADR 0195, design.md, CHANGELOG, operator review.

Verification: column tests for `Room`, a row beside a bar (its steps, row steps, share, and `drop_below` kept, the fill one cell shorter), a fill at its minimum and a row with no fill giving way by the policy, a property test that no row beside a bar is wider than its cells, and one that the lined-up widths are kept whenever the fill has a cell to give (over 100 rooms checked); a tree test that a share keeps its width in a list that overflows and the producer is told `(17, 1)`. Before/after `conspectus tui --snapshot` of 652 frames: every view at 160, 110, 80, 60, and 40 columns (tall, 14 rows, and overflowing heights), the Search view's shapes, and its message hits at 160 columns by 12 to 40 rows; 5 differ, all intended. At 160 columns the message hits' tag is `turn N · message` (20 cells) at every height, as results pass the pane. docs/tui-columns.md's frames are unchanged (its test passes; `just tui-columns` has nothing to regenerate). fmt, clippy `-D warnings`, the isolated suite (4755 passed, 0 failed, 1 ignored), rustdoc `-D warnings`, `cargo build --examples`, and `git diff --check` pass. Frames in /tmp/tuifw-baseline/frames-786-{before,after}.

**Regressions**
- In the Search view's message hits at 160 columns, when the list overflows (heights 12 to 24), a pane line drops its `zsh` command: its name and tag take the widths they have while the list fits, and the command is the first cell to give way, as it already did in the fitting list. The same at 80x40 (stacked), where the tag's 20-cell step starts exactly at the pane's width.

**Consistency wins**
- A Search hit keeps its name and `turn N · message` tag widths as results stream past the pane (it went from 20 cells to `turn 1 · mess…` once the scrollbar appeared); the snippet gives the bar one cell.
- Every other frame captured is unchanged: rows that fit, and rows whose width puts no step or share on the bar's cell, read as before.
<!-- SECTION:FINAL_SUMMARY:END -->
