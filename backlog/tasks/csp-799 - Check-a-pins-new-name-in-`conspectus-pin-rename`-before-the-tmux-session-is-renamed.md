---
id: CSP-799
title: >-
  Check a pin's new name in `conspectus pin rename` before the tmux session is
  renamed
status: Done
assignee:
  - '@claude'
created_date: '2026-10-31 00:20'
updated_date: '2026-10-31 02:12'
labels:
  - cli
  - bug
  - tui-framework
milestone: m-25
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
- [x] #1 `conspectus pin rename` of a live mux pin to a taken or invalid name exits with the store's reason and leaves the tmux session, the pin's overrides, and the pins file as they were
- [x] #2 A test in tests/cli_pin.rs covers a live mux pin renamed to a taken name with a fake tmux
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce first: a test in tests/cli_pin.rs puts a fake `tmux` on PATH (answers `list-sessions` with one live session, records every other call), pins that live mux, adds a second pin, then runs `conspectus pin rename <live> <other pin>` and `pin rename <live> ' padded'` (not a pin id). It asserts each fails with the store's reason, the fake recorded no `rename-session`, and the pins file is byte-identical. Run it and record that it fails.
2. Fix in `PinRenameArgs::run` (src/cli/pin.rs): ask `SlotStore::check_rename` (the check the TUI's store write uses) before discovery, the mux rename plan, the other pins' cascade, or the name override; map its error like the store rename's (legacy slots still get the migrate hint).
3. Verify: the new test and the existing `renaming_a_live_mux_slot_renames_the_tmux_session_with_it` (a free name still renames session, slot, override) pass; full checks.
4. Docs: operations guide (a refused `pin rename` changes nothing), CHANGELOG, operator-review row. No new write path: docs/capabilities.toml unchanged (ADR 0143).
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Reproduced before the fix: `renaming_a_live_mux_slot_to_a_refused_name_changes_nothing` (tests/cli_pin.rs, fake `tmux` script on PATH serving one live session) failed with the fake's log `["rename-session -t live-pin other-pin"]`: the CLI renamed the session, then the store refused the duplicate. Fixed: `PinRenameArgs::run` asks `SlotStore::check_rename` first (error through `missing_slot`, so a legacy pin keeps the migrate hint). The test passes: a taken name and ` padded` (not a pin id) each fail with the store's reason, no `rename-session`, pins file byte-identical; a free name still renames through the fake. Re-checked the test fails with the call removed.

Checks: full isolated suite 4812 passed, 0 failed, 1 ignored (no rerun needed); fmt, clippy (workspace all features; tuikit features off), RUSTDOCFLAGS=-D warnings cargo doc, cargo build --examples, git diff --check pass. docs/capabilities.toml unchanged: no new write path (ADR 0143).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus pin rename` now checks the new name before anything is renamed. Reproduced first with a fake `tmux` on `PATH`: renaming the live mux pin `live-pin` to another pin's name ran `rename-session -t live-pin other-pin`, pinned the name override, and only then failed with ``a pin named `other-pin` already exists``.

- `PinRenameArgs::run` (`src/cli/pin.rs`) asks `SlotStore::check_rename`, the check CSP-778 added and the TUI's store write uses, before discovery, the mux rename plan, the other pins' override cascade, or the pin's own `name` override. It runs for `--no-mux` too, which changes nothing there (the store rename refused the same names first). The error goes through the same legacy lookup as the store rename's, so a legacy pin still gets the migrate hint. Output on success is unchanged; no new write path (ADR 0143), `docs/capabilities.toml` unchanged.
- Test: `renaming_a_live_mux_slot_to_a_refused_name_changes_nothing` (tests/cli_pin.rs) serves one live session from a fake `tmux` script that logs its calls; a taken name and ` padded` (not a pin id) each fail with the store's reason, the fake records no `rename-session`, and `pins.toml` is byte-identical; a free name then renames the session through the fake. It failed before the fix and passes after; the existing real-tmux rename test still passes.
- Docs: operations guide (`pin rename` checks the name first), CHANGELOG (the CSP-778 entry now covers the CLI), operator-review row.

**Regressions:** none.

**Consistency wins:** a refused `pin rename` leaves the tmux session, the overrides, and `pins.toml` as they were, as the TUI's Rename… prompt does. No TUI change.

Verified: full isolated suite 4812 passed, 0 failed, 1 ignored; fmt, both clippy runs, doc, examples, and `git diff --check` pass.
<!-- SECTION:FINAL_SUMMARY:END -->
