---
id: CSP-712
title: Reproduce and fix the suspected scroll and paging defects
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
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
- [ ] #1 Each of the six suspected defects is either reproduced by a failing test and fixed, or recorded in `docs/plans/tui-framework/inventory/` as not reproducible
- [ ] #2 No menu, modal, or pager panics or renders a blank body at 40x10 or after `G` and `PageDown`
- [ ] #3 The row menu keeps its footer at 24 rows, and the `history_row_menu` snapshot is updated
- [ ] #4 Tree paging uses the measured viewport and the scroll offset is clamped to the content length
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
