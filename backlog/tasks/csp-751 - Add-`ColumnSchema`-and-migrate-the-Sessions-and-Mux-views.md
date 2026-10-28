---
id: CSP-751
title: Add `ColumnSchema` and migrate the Sessions and Mux views
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-750
  - CSP-723
  - CSP-725
  - CSP-721
  - CSP-711
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/tui-columns.md
priority: high
type: enhancement
ordinal: 837000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
There is no declarative column description in the TUI: seven `render_*_spans` functions plus inline group-header and fork code make about ten layouts, three alignment structs (`GroupAlign`, `PrAlign`, `MixedAlign`), about 25 width literals, and about 40 hard-coded two-space gutters. Narrow-width behavior is not one policy: Mux rows drop cells and end in an ellipsis, Session rows hard-clip at the pane edge, and the "room remains" cutoff takes three values (above 2, 4, and 8). Ghost muxes emulate the grid with `MixedGrid::Mux` (inventory slice 02, sections 4 and 6).

Scope: Create `ColumnSchema { columns: [Column { key, width: Fixed | Cap(n) | Fill, align, drop_below, priority }] }` and typed cells (text, chip, age, status glyph, right-aligned count, muted secondary with middle ellipsis, snippet), with one measure pass replacing the alignment structs and one narrow-width policy (drop by priority, end in an ellipsis). Migrate the Sessions and Mux views. Regenerate the frames in `docs/tui-columns.md` with `just tui-columns` and edit the prose by hand, as the project requires.

TUI changes:
- Consistency wins: one narrow-width policy for Sessions and Mux; the ghost-mux grid emulation disappears; a test that columns line up comes with the schema
- Regressions or behavior changes: which cells drop first at narrow widths changes for Sessions and Mux rows; frames in `docs/tui-columns.md` change

Tests and checks: The `tui_columns_doc` tests, `consistency_tests/{rendering,docs}.rs`, and the rows and `ui_tests.rs` cases pass or are updated with a note. `--snapshot` at widths 60, 80, 110, and 160.

Blockers: The tree extraction, text, chips, and theme.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `ColumnSchema` and the typed cells exist with one measure pass and one narrow-width policy
- [ ] #2 The Sessions and Mux views render from schemas and `GroupAlign`, `MixedAlign`, and `MixedGrid::Mux` are removed
- [ ] #3 `docs/tui-columns.md` prose and frames match the new behavior and its doc-sync tests pass
- [ ] #4 A cross-view test asserts that columns line up at several widths
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
