---
id: CSP-753
title: Migrate the Search view and hit rows to `ColumnSchema`
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-752
  - CSP-743
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
priority: medium
type: enhancement
ordinal: 839000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A search hit renders in three separate column grids (the modal's entity and content grids and the left pane's hit, pane, and pane-hit rows), and quick select does not reuse the left-pane row renderer. Hit, pane, and pane-hit rows hard-clip at the pane edge, and the age cell is separate (inventory slices 02 and 05).

Scope: Render the Search view's session, hit, pane, and pane-hit rows from schemas that share the column plan used by the search modal, with the age cell and snippet cell from the framework. Keep the lazy hit children and the pane splice by capture time.

TUI changes:
- Consistency wins: search hits look the same in the modal and in the view; narrow-width behavior matches the other views
- Regressions or behavior changes: hit and pane rows stop hard-clipping and drop cells by priority instead

Tests and checks: The `consistency_tests/search_{flat,view}.rs` cases pass or are updated with a note. `--snapshot` of the Search view at 60, 80, and 110 columns.

Blockers: The previous migration and the search palette.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Search view's row kinds render from schemas that share the modal's column plan
- [ ] #2 The three hit-column grids are one definition
- [ ] #3 Hit rows use the framework's age and snippet cells
- [ ] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
