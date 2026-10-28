---
id: CSP-741
title: Extract `Checklist` and drop the `ratatui-cheese` dependency
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-729
  - CSP-727
  - CSP-730
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/adr/0152-tui-framework-dependency-ownership.md
priority: medium
type: enhancement
ordinal: 827000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The multi-select is a thin shim over `ratatui_cheese::multi_select` that keeps a per-frame mirror state and a hand-written `Clone`, and the Clean up modal is another checklist. The cell is silently cut at the edge without an ellipsis, and the list has no select-all and no selected count. ADR 0152 replaces the shim with in-tree code (inventory slice 04).

Scope: Create `Checklist` with labels, preselection, disabled rows, select all and none, a selected count, and `on_confirm(indices)`. Rebuild the Controls sub-editors (harness, mux state, PR state) and the Clean up modal on it. Candidate labels and the record mapping stay in Conspectus. Remove `ratatui-cheese` from the dependency tree (the help use is removed by the help extraction).

TUI changes:
- Consistency wins: select all, select none, and a selected count; long labels end in an ellipsis instead of a hard cut; no mirror state or hand-written `Clone`
- Regressions or behavior changes: none expected

Tests and checks: Checklist tests for toggling, range, disabled rows, select all and none, and confirm. `--snapshot` of the Controls sub-editors and Clean up.

Blockers: The overlay contract, the selection extraction, and the help extraction (the other `ratatui-cheese` use).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `Checklist` replaces the multi-select shim and the Clean up modal's list, with select all and none and a selected count
- [ ] #2 Long labels end with an ellipsis
- [ ] #3 `ratatui-cheese` is removed from the dependency tree
- [ ] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
