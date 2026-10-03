---
id: CSP-525
title: >-
  Pin edit / delete does not resolve a pin when a pinned mux row is selected in
  the mux view
status: Done
assignee: []
created_date: '2026-08-08 03:23'
labels:
  - h-pin-edit-mux
milestone: m-18
dependencies: []
ordinal: 548000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Symptom: on a pinned mux row selected from the Mux view, pressing `R`
  (rename) or `Delete` (via the pins menu / `p`) reports "no editable
  pin `<pin_id>` in current selection" even though the row carries the
  pin's badge and `selected_pin_id()` returns `Some(<id>)`. Selecting
  the same pin's session row from the Sessions view works.
- Root cause: `App::pin_mutation_target` (`src/tui/app.rs:1338`) is the
  single source of truth the pins menu, `CommitRename` pin branch, and
  `remove_pin_action` (`src/tui/runtime.rs:1092`) all consume. It
  matches `RowKind::Pin` and `RowKind::AgentSession(session).pin_id`
  but omits the parallel `RowKind::MuxSession(mux).pin_id` case that
  `row_pin_id` (`src/tui/app.rs:3188`) and `selected_pin_id`
  (`src/tui/app.rs:1581`) already handle. So `selected_pin_id()`
  returns `Some(...)`, `pin_mutation_target()` returns `None`, and the
  caller falls through to the "not selected" toast.
- Fix: extend the `match &row.kind` arm to resolve
  `RowKind::MuxSession(mux)` via `mux.pin_id.clone()?` (mirroring the
  AgentSession arm) so the pin lookup in `database.snapshot().pins`
  fires the same way as it does from a session row.
- Tests: regression modeled on
  `pins_context_seeds_pin_mutation_target_from_selected_pin_row`
  (`src/tui/app_tests.rs:366`), but selecting the mux row that backs
  the pin. Assert `pin_target` resolves to the same
  `PinMutationTarget` fields the session-row test asserts.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): landed in `084967f`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-EDIT-MUX-001`
