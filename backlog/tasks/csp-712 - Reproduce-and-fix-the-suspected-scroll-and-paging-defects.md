---
id: CSP-712
title: Reproduce and fix the suspected scroll and paging defects
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 06:13'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
priority: medium
type: bug
ordinal: 798000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The inventory found six scroll and paging problems by reading the source; none has been reproduced. Fixing them before the extraction keeps the new `scroll`, `Pager`, `ActionMenu`, and `TreeView` from inheriting or hiding them. The suspected defects: `G` and `PageDown` in the value modal and the Messages detail pane set an unclamped scroll offset, so the wrapped body can render blank; three hand-rolled menus call `clamp(8/10/5, area.height)`, which panics on a terminal shorter than the minimum; the row menu cannot scroll, and the committed `history_row_menu` snapshot already loses its footer at 24 rows; tree PageUp and PageDown step by terminal height minus 2 instead of the measured viewport, and the tree scroll offset is never clamped to the content length; the main help over-scroll is never clamped back; the viewer help pages by the transcript's viewport, skipping at least 3 lines per page.

Scope: For each suspected defect, write a failing test or a `--snapshot` reproduction first. Fix the ones that reproduce, with the smallest change in the current code; record the ones that do not reproduce in the inventory file and drop them. Write the tests at the behavior level (what the operator sees) so they survive the later extraction and become its regression suite.

TUI changes:
- Consistency wins: no blank body after `G` or `PageDown`; no panic on a short terminal; menus scroll and keep their footer at 24 rows; paging moves by the measured viewport
- Regressions or behavior changes: paging distance changes in the tree and the help page (the viewport replaces terminal height minus 2 and the fixed 8-line step)

Tests and checks: Each fix has a test that fails before and passes after. Run the menus, value modal, Messages, help, and tree at 80x24 and at 40x10 with `conspectus tui --snapshot`.

Blockers: Reading only so far: if a defect does not reproduce, close it with a note rather than inventing a fix.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Each of the six suspected defects is either reproduced by a failing test and fixed, or recorded in `docs/plans/tui-framework/inventory/` as not reproducible
- [x] #2 No menu, modal, or pager panics or renders a blank body at 40x10 or after `G` and `PageDown`
- [x] #3 The row menu keeps its footer at 24 rows, and the `history_row_menu` snapshot is updated
- [x] #4 Tree paging uses the measured viewport and the scroll offset is clamped to the content length
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Write behavior-level reproductions first in a new consistency suite (`src/tui/consistency_tests/scrolling.rs`), driving keys the way the live loop does (draw a frame, then route the key), at 80x24 and 40x10 and below the menus' minimum heights:
   - value modal and Messages detail: `G` / `PageDown` past the end keep text on screen; `k` after `G` moves at once; `PageDown` moves by the rows drawn
   - row, mux, and worktree menus: no panic on terminals shorter than their minimum height
   - row menu: footer stays visible at 24 rows, and the cursor entry stays visible as it moves down
   - tree: `PageDown` moves the selection by the measured tree viewport; the scroll offset never leaves a blank tail after the tree shrinks
   - main help: over-scroll at the end is clamped back (`k` after `G`, `j` past the end); `PageDown` moves by the rows drawn
   - viewer and pane viewer help: `PageDown` moves by the help page's own body height
2. Run them, record which reproduce; any that does not goes into a "Verification (CSP-712)" section of the inventory file.
3. Fix the ones that reproduce with the smallest change:
   - pagers (value modal, Messages detail, help, viewer/pane-viewer help): the renderer reports the rows it drew and how far it can scroll; the draw path writes that back (clamp the offset, remember the page height); page keys move by the remembered page height. The value modal and Messages detail pre-wrap their text so the clamp is exact.
   - menus: replace `clamp(min, area.height)` with `max(min).min(area.height)`; the row menu keeps the footer on its last rows and scrolls entries to keep the cursor visible.
   - tree: App remembers the measured tree viewport from `LeftViewportChanged`; the key path pages by it; `adjust_left_scroll` clamps the offset to `rows - viewport`.
   - snapshot driver: draw a frame before each scripted key, as the live loop does, so paging by the drawn viewport behaves the same in `--snapshot`.
4. Update the tests and snapshots that encoded the old behavior (`history_row_menu`, fixed-8 paging, unclamped `G`), deliberately.
5. Verify with `conspectus tui --snapshot` at 80x24 and 40x10 (menus, value modal, Messages, help, tree); run fmt, clippy, the isolated suite, doc build, `git diff --check`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Reproductions (new suite `src/tui/consistency_tests/scrolling.rs`, keys sent the way the live loop sends them): all six reproduce before any fix.
1. Value modal: `G` and `PageDown` on a short value draw a blank body at 80x24 and 40x10; Messages detail goes blank after paging past its end; paging moves 8 lines while the modal shows 19 (80x24) or 7 (40x10).
2. Clamp panics: worktree menu below 5 rows, mux menu below 10, row menu below 8 (`min > max`).
3. Row menu: footer and cursor entry clipped at 80x14 and 40x10 (and the committed `history_row_menu` frame at 100x24).
4. Tree: `PageDown` moves 22 rows in a 20-row pane at 120x24; collapsing the last group from the end leaves 1 row drawn with 6 scrolled off above and the rest blank.
5. Help: `k` after `G` (and after extra `j`) does not move; `PageDown` moves 8 lines of a 17-row body.
6. Viewer and pane viewer help at 80x16: `PageDown` moves 12 lines of a 9-row help body.

Fixed in four code commits (tree paging and clamp; menu heights and row-menu scroll; pager fit write-back; snapshot driver keys) plus docs. Verification on the final tree: `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean; isolated `cargo test --workspace --all-targets --all-features` 3481 passed, 0 failed, 1 ignored; isolated `cargo nextest run --workspace --all-targets --all-features` 3481 passed, 1 skipped; `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps` clean; `git diff --check` clean. `conspectus tui --snapshot` (showcase fixture) before/after at 80x24 and 40x10: tree `<PageDown>` moves 2 rows in the 2-row stacked tree at 40x10 (was: to the end); help `?Gk` scrolls up one line (was: unchanged); help `?<PageDown>` moves 6 lines at 40x10; row menu at 40x7 and 40x4 draws with its cursor and footer (was: panic `min > max. min = 8, max = 7`); mux menu at 40x9/40x7/40x4 draws (was not openable in snapshot mode). The value modal could not be opened from the showcase fixture (no core field is truncated at these widths, and `o` on a truncated field in Expanded Node Detail does not open it, filed as CSP-771), so its 80x24 and 40x10 frames are checked by the App-level tests instead. The showcase log is empty, so Messages paging is likewise checked by those tests.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
All six suspected defects reproduced, each through a failing behavior test in the new `src/tui/consistency_tests/scrolling.rs` (14 tests that send keys the way the live loop does: draw, then key, at 80x24 and 40x10), and all six are fixed. None needed a "not reproducible" note; each inventory slice gained a "Verification (CSP-712)" section.

1. Value modal and Messages record: `G` and `PageDown` set an unclamped offset and drew a blank body. Each draw now reports the rows it showed and how far its text can scroll (`popup_frame::ScrollFit`); the draw path writes that back (clamped offset, page height). The text is wrapped by the widget so the clamp is exact.
2. Row, mux, and worktree menus panicked below 8, 10, and 5 rows (`clamp` with min > max), also under `--snapshot`. Heights are now `max(min).min(area.height)`.
3. Row menu could not scroll and the `history_row_menu` frame lost its footer at 24 rows. The footer keeps the last rows and entries scroll to the cursor. Snapshot updated deliberately.
4. Tree `PageUp`/`PageDown` stepped by terminal height minus 2, and the offset was never clamped (blank tail after a collapse). The App keeps the height `LeftViewportChanged` reports and pages by it; `adjust_left_scroll` clamps to `rows - viewport`.
5. Main help over-scroll was never taken back: fixed by the same write-back as 1.
6. Viewer and pane-viewer help paged by the transcript viewport (12 lines through a 9-row help at 80x16). They now record and page by their help body.

The `--snapshot` driver now draws before each key, as the live loop does, and passes keys to the value modal and menus (picks and `y` still skipped).

Verified: fmt, clippy (`-D warnings`), the isolated suite (3481 passed, 0 failed, 1 ignored), nextest (3481 passed), docs (`RUSTDOCFLAGS='-D warnings'`), `git diff --check`, and before/after `conspectus tui --snapshot` frames (see notes).

**Regressions**
- Tree `PgDn`/`PgUp` move by the rows the tree draws, not the terminal height minus 2 (20 rather than 22 at 120x24; 2 rather than 8 in the stacked tree at 40x10).
- Help, value modal, and Messages record `PgDn`/`PgUp`/`Space` move by the rows on screen, not 8 (17 for help at 80x24, 19 for the value modal, 6 for help at 40x10).
- Viewer and pane-viewer help pages are the help's rows, not the transcript's viewport.
- The value modal and Messages record wrap with the help page's word wrap, not ratatui's, so a line can break one word earlier at an exact fit.

**Consistency wins**
- No blank body after `G` or paging past the end; the next key up moves at once (value modal, Messages record, main help).
- No menu panics on a short terminal; every modal shows its body at 40x10.
- The row menu scrolls to its cursor and keeps its key hints, including at 24 rows.
- A shrinking tree leaves no blank rows below it with rows hidden above.
- Every pager pages by the viewport it drew (house rule 12).

Follow-up filed: CSP-771 (`o` in Expanded Node Detail). Not done here: the mux menu still clips its footer below 10 rows (`ActionMenu` gives every menu scrolling).
<!-- SECTION:FINAL_SUMMARY:END -->
