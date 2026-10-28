---
id: CSP-750
title: Extract `TreeState<Id>` and `TreeView<R>` with app-rendered lines
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-724
  - CSP-727
  - CSP-725
  - CSP-723
  - CSP-714
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/adr/0151-tui-framework-control-set-and-house-rules.md
priority: high
type: enhancement
ordinal: 836000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The left pane is not a widget. `draw_left_panel(&mut App, ...)` reads about 20 `App` accessors, builds one `Line` per visible row, and hands the lot to a `Paragraph`. Tree state is five loose fields on `App` mutated by `app/tree.rs` and reducer arms, selection retention is duplicated between `set_data` and `set_tree`, three expand and collapse scans repeat, and the draw dispatches `Msg::LeftViewportChanged` mid-render. `RowKind` and `RowId` are also read by header counts, preview selection, the quick-select index, the keymap, and actions, so a generic control cannot own them (inventory slice 02, sections 0 to 3).

Scope: Create `TreeState<Id>` (selection retained by id with the nearest-duplicate tiebreak, expansion set, scroll, measured viewport, paging, home and end) and `TreeView<R>` (pane chrome slots for the tab title, header line, footer banner, and empty state, the body, and the scrollbar gutter), with rows generic over `Id` and cell content supplied by the app as a `Fn(&R, RowCtx) -> Line`. This is the stepping stone: the opinionated column layout stays in app code until the column schema stories. The renderer reports the measured viewport itself, so the mid-draw message goes away. Domain lookups in the renderer (`pinned_nodes()`, `presence()`, `row_is_frozen`) move to the app's row producer.

TUI changes:
- Consistency wins: PageUp and PageDown move by the measured viewport; no blank tail past the content; one retention routine
- Regressions or behavior changes: paging distance changes (was terminal height minus 2)

Tests and checks: The `rows` tests, `ui_tests.rs` scroll tests, and `consistency_tests/{state,rendering}.rs` pass. Property tests for selection retention across rebuilds. `--snapshot` of the Sessions, Mux, and Pinned views at 80x24 and 40x10.

Blockers: Scroll, selection, chips, and text, plus the left-pane defect task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `TreeState<Id>` owns selection, expansion, scroll, viewport, and paging, and the five loose `App` fields and the duplicated retention and expand/collapse code are removed
- [ ] #2 `TreeView<R>` renders the pane chrome slots, the body from app-supplied lines, and the scrollbar gutter
- [ ] #3 No renderer dispatches a message during `draw`
- [ ] #4 `RowKind` and `RowId` stay in Conspectus and the framework control is generic over `Id`
- [ ] #5 Domain lookups no longer run inside the renderer
- [ ] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
