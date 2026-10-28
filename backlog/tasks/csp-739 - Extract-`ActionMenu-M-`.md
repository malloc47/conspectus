---
id: CSP-739
title: Extract `ActionMenu<M>`
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
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/adr/0151-tui-framework-control-set-and-house-rules.md
priority: medium
type: enhancement
ordinal: 825000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Four menus are one control: the row menu (`.`), the mux menu (`m`), the worktree menu's list, and the pin unpin menu. Each has its own heading and width (the mux menu is hand-framed at 62 cells), the row menu cannot scroll, and the committed `history_row_menu` snapshot already loses its footer at 24 rows. One entry, Forget, confirms by an inline "press Enter again"; the others do not (inventory slice 04).

Scope: Create `ActionMenu<M>` with entries `{label, detail, shortcut, enabled, confirm: None | Inline, outcome: M}`, scrolling, generated footer, and the shared selection and frame. Turn the mux menu into data fed to it. The entry policy (`row_menu_for`, `pin_management_entries`) and the `Msg` mapping stay in Conspectus. Apply house rules 1 to 5.

TUI changes:
- Consistency wins: menus scroll and keep their footer on short terminals; one heading and width policy; `q` closes a menu; an inline-confirm entry shows what Enter will do
- Regressions or behavior changes: the mux menu's heading and width change to the shared policy; menus wrap at the ends

Tests and checks: Menu tests for scrolling, disabled entries, inline confirm, and shortcuts. `--snapshot` of each menu at 80x24 and 40x10. The `consistency_tests/menus.rs` entity-equivalence checks pass.

Blockers: The overlay contract and the selection extraction.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `ActionMenu<M>` replaces the row menu, the mux menu widget, the worktree menu's list, and the pin unpin menu
- [ ] #2 The menu scrolls and keeps its generated footer at 40x10 and 80x24
- [ ] #3 An entry can require an inline second Enter, and the entry says so
- [ ] #4 The mux menu widget file is deleted and its two verbs are data
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
