---
id: CSP-758
title: 'Extract `TabBar`, `PriorityLine`, and `EmptyState`'
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 08:10'
labels:
  - tui-framework
  - tuifw-chrome
milestone: m-25
dependencies:
  - CSP-723
  - CSP-725
  - CSP-757
  - CSP-716
  - CSP-790
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 844000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The left pane's tab strip joins view labels with ` · ` and probably clips at 100 to 107 columns with no elision. The header shows chips with priority-drop fitting only in the status bar. Twelve hand-written empty-state messages share a slot but no code. Breadcrumb elision and tab elision are the same fit problem (inventory slice 07).

Scope: Create `TabBar` (active accented, rest secondary, separated, with elision), `PriorityLine` and `StatusChip` (pieces with priorities that drop in order to fit), and `EmptyState` (a message slot with a standard look). The header composition and the empty-state copy stay in Conspectus. Do not add a minimum-size screen (CSP-319 and CSP-190 stay open) but leave the seam.

TUI changes:
- Consistency wins: the tab strip fits or elides at every width; the header degrades by priority like the status bar
- Regressions or behavior changes: none expected

Tests and checks: `--snapshot` at widths 40, 60, 80, 100, 107, and 120 for the tab strip and header. Tests for each elision step.

Blockers: Text, chips, and the frame.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `TabBar`, `PriorityLine`, `StatusChip`, and `EmptyState` are in the framework
- [ ] #2 The tab strip and header never clip mid-label at any width from 40 to 120
- [ ] #3 The 12 empty-state messages use `EmptyState` and keep their copy
- [ ] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
