---
id: CSP-760
title: Design and extract the scoped global keymap and generate the main help from it
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-chrome
milestone: m-25
dependencies:
  - CSP-759
  - CSP-730
  - CSP-710
documentation:
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
priority: medium
type: enhancement
ordinal: 846000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The global keymap (`KEYBINDINGS`) has a single `KeyMode`, 58 `help_text` strings that are never rendered, seven keys outside the table, and a duplicate-key test that misses shadowed rows. The main `?` page is 48 hand-written rows with a one-directional drift test. Several keys change meaning by focus (`F`, Enter, Backspace). It is wired into `Action`, `Msg`, and `View`, so the framework designs a scoped keymap fresh and Conspectus adopts it (inventory slice 06, section 1).

Scope: Add a scoped keymap to `tuikit::keys` (modes: global, pane focus, overlay) whose rows carry help text that renders, with collision and shadow detection at construction and stable ids that leave room for user remapping later. Adopt it in Conspectus, declare the same-key-two-meanings in one table, and generate the main help page from the table with the hand-written prose rows kept as annotations.

TUI changes:
- Consistency wins: the main help page and the status bar always match the keys; shadowed and duplicate keys fail at construction
- Regressions or behavior changes: the wording of some main-help rows changes as prose rows become table descriptions

Tests and checks: The keymap drift tests are replaced by two-way tests. `--snapshot` of the main help page.

Blockers: The status bar and help extractions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The scoped keymap has modes, rendered help text, and collision and shadow detection, and the global table uses it
- [ ] #2 The main help page is generated from the table, with prose rows kept as annotations
- [ ] #3 Rows have stable ids, so a later user remap needs no table change
- [ ] #4 The two-way drift test replaces the one-directional test
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
