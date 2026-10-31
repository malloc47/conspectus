---
id: CSP-799
title: >-
  Check a pin's new name in `conspectus pin rename` before the tmux session is
  renamed
status: To Do
assignee: []
created_date: '2026-10-31 00:20'
labels:
  - cli
  - bug
dependencies: []
references:
  - src/cli/pin.rs
  - src/pin_slots.rs
priority: low
type: bug
ordinal: 885000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while fixing CSP-778, the same defect in the TUI. `PinRenameArgs::run` (`src/cli/pin.rs`) renames a live mux pin's tmux session through `execute_mux_rename_plan` and pins the `name` override before `pin_ops::rename` asks the store whether the new name is free. So `conspectus pin rename live-pin other-pin`, where `other-pin` already exists, renames the session, sets the override on `live-pin`, and then fails with `a pin named `other-pin` already exists`.

The TUI now asks `SlotStore::check_rename` (the names `SlotStore::rename` would refuse, checked without writing) before it touches the session; the CLI can do the same. This changes only what a failing `pin rename` leaves behind, not its output on success.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `conspectus pin rename` of a live mux pin to a taken or invalid name exits with the store's reason and leaves the tmux session, the pin's overrides, and the pins file as they were
- [ ] #2 A test in tests/cli_pin.rs covers a live mux pin renamed to a taken name with a fake tmux
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
