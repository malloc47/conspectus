---
id: CSP-727
title: 'Extract `selection`: one list cursor, highlight, and wrap-or-clamp rule'
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-foundation
milestone: m-25
dependencies:
  - CSP-721
  - CSP-722
  - CSP-724
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
  - docs/adr/0151-tui-framework-control-set-and-house-rules.md
priority: high
type: enhancement
ordinal: 813000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Lists disagree on how selection looks and behaves. Four highlight looks exist: `►` plus BOLD (menus), `> ` plus REVERSED (Controls, Picker, Search, forms), accent only, and the themeable `selection_active` (Messages, left pane). At least 6 widgets implement the highlight independently, the unfocused explorer shows no cursor, and menus clamp at the ends while longer lists wrap. House rules 1, 2, and 10 settle these. Doing this once, ahead of the control extractions, means each later control story does not change the look again.

Scope: Create `tuikit::selection` with the cursor marker (`marks::POINTER` in a 2-cell gutter), the full-row highlight from `selection_active` and `selection_inactive`, ghost and pending row styles, and the wrap-or-clamp stepping rule (short choice lists wrap; scrollable content clamps). Migrate every in-place site: Controls, Picker, Search, forms, the menus, Messages, the explorer, and the left pane.

TUI changes:
- Consistency wins: one selection look in every list; the marker keeps selection visible without colour; the unfocused explorer shows its cursor; menus wrap like the other choice lists
- Regressions or behavior changes: menus, Controls, Picker, Search, and the forms change their cursor glyph and highlight; menus now wrap at the ends instead of stopping; the right pane gains a visible inactive cursor

Tests and checks: `conspectus tui --snapshot` of every list at 80 and 40 columns, with focus on and off. A test fails on a hand-written `REVERSED` cursor in framework source.

Blockers: The theme, glyph, and scroll extractions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `tuikit::selection` provides the marker, the active and inactive highlight, the ghost and pending styles, and the wrap-or-clamp rule
- [ ] #2 Every list and menu uses it, and no list hand-writes a `REVERSED` cursor or a `> ` or `►` literal
- [ ] #3 Short choice lists wrap and scrollable content clamps, documented in the reference page
- [ ] #4 The right pane shows an inactive cursor when unfocused
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
