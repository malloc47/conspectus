---
id: CSP-778
title: Check a pin's new name in the Rename prompt before anything is renamed
status: To Do
assignee: []
created_date: '2026-10-29 08:42'
labels:
  - tui-framework
  - tuifw-text
milestone: m-25
dependencies: []
references:
  - src/tui/app/overlays.rs
  - src/tui/runtime/executor.rs
  - docs/adr/0170-tui-pin-edit-form-is-a-form-with-override-rows.md
priority: low
type: bug
ordinal: 864000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A pin has two rename UIs (ADR 0170 keeps both): the **Rename…** prompt (`open_pin_rename`, `StoreOp::RenameSlot`) and the first row of the pin edit form (`StoreOp::SavePin` with a rename). The form checks the new name with `pin_slots::validate_id` and against the other pins before it saves, and says why on its error line. The prompt refuses only a blank name; the store refuses the rest after the fact, as a toast.

Found by reading (not run): for a live mux pin, `execute_rename_slot` renames the tmux session (and sets the name override) before `pin_ops::rename` checks for a duplicate pin id, so renaming a live mux pin through the prompt to another pin's name renames the session and then fails to rename the pin. The edit form never reaches that path with a taken name.

The host already has the form's check (`pin_edit::check_name`); the prompt's `on_confirm` can refuse with the same reasons through `PromptResult::Reject`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Rename… prompt refuses an invalid pin id and a name another pin has, in the prompt, with the same reasons the edit form gives
- [ ] #2 A refused prompt rename changes nothing: no tmux rename, no override, no store write
- [ ] #3 A test covers a live mux pin renamed through the prompt to a taken name
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
