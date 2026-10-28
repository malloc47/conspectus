---
id: CSP-743
title: Move quick select and content search onto `Palette`
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-742
  - CSP-723
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/adr/0149-two-level-search-current-and-wider-context.md
priority: high
type: enhancement
ordinal: 829000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`widgets/search.rs` is 1,523 lines holding two modes: quick select (`/`) with a Current and All scope, and content search with an options pane, a debounce and progress state machine, and history paging. Search results appear in three separate hit-column grids and quick select does not reuse the left-pane row renderer. The hit chip width is hard-coded at 10 (inventory slice 05, sections 2.9 and 6).

Scope: Rebuild both modes as `Palette` instances with Conspectus row types and a shared `ColumnPlan` for the hit columns (the same plan the Search view uses later). The ranking, the entity index, outcome words, content kinds, harness badges, and history paging stay in Conspectus. Keep ADR 0147 and ADR 0149 semantics exactly: the search behavior does not change.

TUI changes:
- Consistency wins: one hit-column plan instead of three grids; the hit chip honors `theme.badge_width`
- Regressions or behavior changes: none expected

Tests and checks: The existing `search_quick_tests.rs`, `search_content_tests.rs`, `consistency_tests/search_*.rs`, and `tests/cli_search.rs` pass unchanged. `--snapshot` of both modes at 80x24 and 40x10.

Blockers: `Palette`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Quick select and content search are `Palette` instances and `widgets/search.rs` shrinks to the adapter
- [ ] #2 A shared column plan drives the modal's hit rows
- [ ] #3 Search behavior is unchanged: every search test passes without edits to expectations
- [ ] #4 The hit chip width comes from the theme
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
