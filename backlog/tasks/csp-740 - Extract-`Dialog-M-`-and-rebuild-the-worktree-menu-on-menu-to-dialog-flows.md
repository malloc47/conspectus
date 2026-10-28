---
id: CSP-740
title: Extract `Dialog<M>` and rebuild the worktree menu on menu-to-dialog flows
status: To Do
assignee: []
created_date: '2026-10-27 20:27'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-739
  - CSP-734
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
priority: medium
type: enhancement
ordinal: 826000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Confirmation has four idioms. The worktree menu is a mode machine in one 677-line widget: a list, a text prompt, three y/n confirm sub-screens (remove, merge, prune), and an `m`/`d` close-down choice, with about 215 lines that query the graph. Ctrl-C means close-everything in Controls but back-one-level in this menu. The Clean up modal has its own empty-state notice (inventory slice 04).

Scope: Create `Dialog<M>` in three forms: confirm (`y`/Enter, `n`/Esc), choice (named verbs), and notice. Rebuild the worktree menu as an `ActionMenu` opening dialogs and a `TextPrompt` on the overlay stack, with Esc stepping back one level. The graph-querying context capture, the `WorktreeAction` policy, and the `Msg::CommitWorktree*` mapping stay in Conspectus. The Clean up empty notice becomes a `Dialog` notice.

TUI changes:
- Consistency wins: one confirm keymap in every dialog; a stack replaces the mode machine
- Regressions or behavior changes: Ctrl-C in the worktree menu closes it instead of going back one level

Tests and checks: Tests for each worktree flow (create, remove, merge, prune, close-down) through the stack. The `worktree_menu_tests.rs` cases pass or are updated with a note. `--snapshot` of each dialog.

Blockers: `ActionMenu` and `TextPrompt`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `Dialog<M>` provides confirm, choice, and notice forms with one key table
- [ ] #2 The worktree menu is built from `ActionMenu`, `Dialog`, and `TextPrompt` on the overlay stack, and its mode machine is removed
- [ ] #3 The graph-querying context and the worktree policy stay in Conspectus
- [ ] #4 The Clean up empty-state notice is a `Dialog` notice
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [ ] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [ ] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
