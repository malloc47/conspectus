---
id: CSP-687
title: Reorder pin slots from the TUI
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
updated_date: '2026-10-31 08:16'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-679
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: low
type: feature
ordinal: 745000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The slot store keeps an order and the Pinned view sorts by it, but at first only `pin move` on the CLI changes it. The operator wants to arrange slots from the TUI.

- Scope: move-up / move-down (and move to top or bottom) actions on Pinned rows, menu-first with accelerators, written through the slot store.
- Tests: action and persistence tests.
- Manual checks: reorder slots in the Pinned view and confirm the order survives a restart.
- Blockers: the Pinned view ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Slots can be reordered from the Pinned view, and the order persists in `pins.toml`
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): where the pieces go now. A Pinned row's menu entries are data in `actions::row_menu_for` (`src/tui/actions.rs`), drawn by `tuikit::action_menu::ActionMenu` (ADR 0171). An accelerator is a binding in the scoped keymap (`src/tui/keybindings.rs`, ADR 0192), whose build refuses a key that overlaps another in its scope, and which can carry `available_when` so the status bar and the `?` page dim a move that cannot act (the first slot moving up). The store write declares its lifecycle, and its undo, in `tui::effect::lifecycle`. Help and status hints are generated from the keymap, so they need no hand edits.
<!-- SECTION:NOTES:END -->
