---
id: CSP-631
title: 'Keep the mux backend when the TUI creates, adopts, or edits a pin'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-18 01:27'
updated_date: '2026-10-18 01:43'
labels:
  - h-ext
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: high
type: bug
ordinal: 689000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI writes every pin it creates or edits with `mux.backend = "tmux"` (`src/tui/runtime/pin_store.rs:189` for create and adopt, `:295` for edit), whatever mux the pin belongs to. Two consequences:

- Saving any edit to an existing zellij pin from the TUI rewrites its stored backend to `tmux`, silently corrupting the operator's `.conspectus.toml`.
- Adopting a zellij mux row from the TUI writes a tmux pin, and renames the source mux through the tmux runner (`apply_pin_adopt_mux_rename`, `pin_store.rs:146`), which acts on whatever tmux session shares the name.

The duplicate-mux preflight (`pin_store.rs:349`) also compares only against tmux entries. Found while fixing CSP-617 and CSP-618; the CLI paths already keep the backend.

- Scope: carry the backend through the TUI create, adopt, and edit requests. Create defaults to tmux; adopt takes the selected mux row's backend; edit keeps the pin's stored backend. Route the adopt-time rename through that backend's runner, reporting an unsupported backend instead of calling tmux. Make the duplicate-mux preflight compare backend, name, and socket together.
- Tests: editing a zellij pin keeps `backend = "zellij"` in the written TOML; adopting a zellij row writes a zellij pin and never calls the tmux runner; creating a pin still writes tmux; preflight rejects a same-name same-backend duplicate and allows a same-name pin on another backend.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Editing a pin from the TUI preserves its stored `mux.backend`
- [x] #2 Adopting a mux row from the TUI writes the row's backend and renames through that backend only
- [x] #3 Creating a pin from the TUI still writes `backend = "tmux"`
- [x] #4 The duplicate-mux check compares backend, name, and socket
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `mux_backend` to `PinCreateDefaults` (manual `Default`: tmux), `PinCreateRequest`, `PinEditRequest`, and `PinMutationTarget`.
2. Sources: the mutation target takes the snapshot pin's backend; adopt defaults take the selected mux row's backend; other create defaults stay tmux. The create form state carries it from defaults; the edit, rebind, and rename-from-row requests carry the target's.
3. `pin_store` writes the request backend for create and edit, and its duplicate-mux check compares backend, name, and socket.
4. Adopt rename runs through the adopted mux's backend (the loop's tmux runner for tmux, the system runner otherwise).
5. Tests for each path; confirm the store tests fail with the old hard-coded backend and tmux-only check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- Found a third path while working: the TUI pin rename (`R`, `app.rs` `PinEdit` from the pins context) also wrote `backend = tmux`. It now carries the target's backend like edit and rebind.
- `PinCreateDefaults` lost `#[derive(Default)]` for a manual impl so the default backend is tmux, not an empty string; new-variation defaults use `..PinCreateDefaults::default()`.
- `apply_pin_adopt_mux_rename` uses the loop's injected runner only when the request's backend matches it; zellij gets `SystemZellij`, whose rename is unsupported, so the status reports the failed rename and tmux is never called.
- Tests: `edit_and_rebind_forms_keep_the_pins_mux_backend`, `new_pins_default_to_tmux`, `pin_adopt_defaults_take_the_selected_muxs_backend` (also asserts create defaults stay tmux), `write_pin_edit_keeps_a_zellij_pin_on_zellij`, `write_pin_edit_checks_mux_collisions_within_the_same_backend`, `pin_adopt_mux_rename_never_renames_a_zellij_mux_through_tmux`. With the old hard-coded write the keep-zellij test fails; with the old tmux-only duplicate check the collision test fails.
- Validation: `just check` green (2419 tests).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The TUI no longer rewrites pins to tmux. Create, adopt, edit, rebind, and rename now carry the pin's mux backend: new pins default to tmux, adopt takes the selected mux row's backend, and edits keep the stored one, so saving a zellij pin leaves it a zellij pin. Adopt renames the source mux only through its own backend, and the duplicate-mux check compares backend, name, and socket. Verified with new tests that fail against the old code and `just check` (2419 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
