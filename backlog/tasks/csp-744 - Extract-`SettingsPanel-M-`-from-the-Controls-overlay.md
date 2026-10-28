---
id: CSP-744
title: Extract `SettingsPanel<M>` from the Controls overlay
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-741
  - CSP-734
  - CSP-739
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/adr/0031-tui-filter-view-switching-and-per-view-state.md
priority: medium
type: enhancement
ordinal: 830000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Controls overlay is 1,344 lines: a sectioned panel with 24 rows, five row kinds (choice, checkbox, editor, cycle, button), and three sub-editor types, applying changes as you go. About 25 to 30% is generic (landable-row cursor, sections, row kinds, sub-editors, footer, scroll-to-cursor); the rest is Conspectus settings. Ctrl-C closes everything here but steps back in the worktree menu, it clones an 11-field context, and it has no paging keys despite about 30 rows (inventory slice 04).

Scope: Create `SettingsPanel<M>` with sections, the five row kinds, sub-editors that open a `Checklist` or a `TextPrompt`, scroll-to-cursor, and a generated footer; keep the "changes apply as you make them, Esc keeps them" behavior as first-class. The 24 rows, `ControlsContext`, and the filters stay in Conspectus. Apply the close-key and paging rules.

TUI changes:
- Consistency wins: paging keys on a 30-row panel; generated footer; Ctrl-C and Esc follow the shared rule
- Regressions or behavior changes: none expected

Tests and checks: The `controls_tests.rs` cases pass or are updated with a note. `--snapshot` of the panel and each sub-editor at 80x24 and 40x10.

Blockers: `Checklist`, `TextPrompt`, and `ActionMenu`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `SettingsPanel<M>` provides sections, five row kinds, sub-editors, and scroll-to-cursor
- [ ] #2 The Controls overlay is a `SettingsPanel` instance and reads its context through `&Cx`
- [ ] #3 `PgUp`, `PgDn`, `Home`, and `End` work on the panel
- [ ] #4 The apply-as-you-go behavior is preserved and documented
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
