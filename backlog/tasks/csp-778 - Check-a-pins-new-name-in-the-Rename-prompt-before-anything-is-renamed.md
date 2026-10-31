---
id: CSP-778
title: Check a pin's new name in the Rename prompt before anything is renamed
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29 08:42'
updated_date: '2026-10-31 00:26'
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
- [x] #1 The Rename… prompt refuses an invalid pin id and a name another pin has, in the prompt, with the same reasons the edit form gives
- [x] #2 A refused prompt rename changes nothing: no tmux rename, no override, no store write
- [x] #3 A test covers a live mux pin renamed through the prompt to a taken name
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Make the executor's slot rename testable: split `execute_rename_slot` into `rename_slot_with(snapshot, store, tmux, slot, new_name)`, a pure function over its inputs, plus the wrapper that finds the store and logs (no behavior change).
2. Reproduce first: a test in a new `executor_rename_tests.rs` opens the Rename… prompt on the live mux pin of the Pinned fixture world, types another pin's name, confirms, and runs the effects through `rename_slot_with` with a `FakeTmux` and a temp store; it asserts no tmux rename, no override, no store write. Run it and record that it fails.
3. Fix in the prompt: `open_pin_rename` refuses with the edit form's `check_name` (the pin id rules and the other pins' names, from one `App` helper both openers use), so the prompt says the form's reasons on its error line. Fold the blank case into `check_name` with the prompt's documented reason (`pin: nothing entered`) so both rename UIs give one set of reasons (consistency-first, ADR 0151).
4. Fix the order in the executor too: check the new name against the store before renaming the tmux session or setting the override, so a name taken since the prompt opened changes nothing either (shared `SlotStore::check_rename`).
5. Tests: the reproduction passes; the executor alone refuses a taken name without touching tmux; a free name still renames the session, the slot, and sets the override; the consistency text-entry check shows the taken-name reason inside the prompt and the form; `check_name` refuses an invalid id.
6. Docs: operations.md and CHANGELOG for the prompt's new refusals; operator-review row; file the CLI `pin rename` ordering (same defect outside the TUI) as a follow-up.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Reproduced before the fix: `executor_rename_tests.rs` opens the Rename… prompt on the fixture's live mux pin `conspectus-main`, confirms `config-agent3` (another pin), and runs the write through `rename_slot_with` with a `FakeTmux` and a temp store. It failed with `tmux.rename_calls() == [(None, "conspectus-main", "config-agent3")]`: the session was renamed, then the store refused the duplicate (`rename failed: a pin named `config-agent3` already exists`). The executor-only test failed the same way.

Fixed: the prompt refuses with `pin_edit::check_name` (shared with the edit form; blank folded in as `pin: nothing entered`), and `rename_slot_with` asks `SlotStore::check_rename` before renaming the session. Verified: the reproduction and executor tests pass, `the_name_check_refuses_blank_invalid_and_taken_names`, `check_rename_refuses_what_rename_would_without_writing`, and the consistency check `a_taken_pin_name_says_why_in_the_prompt_and_in_the_edit_form`. Frames: /tmp/tuifw-baseline/frames-778-{before,after} (100x30, 50x16): only prompt-taken and form-blank differ. Full suite (isolated runner) 4799 passed, 0 failed, 1 ignored after one rerun for the known `discovery::forge` timing flake. fmt, clippy (workspace all features, tuikit features off), doc, examples, diff --check pass. Filed CSP-799 for the CLI's `pin rename`, which has the same order.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The **Rename…** prompt now checks a pin's new name before anything is renamed. Reproduced first: renaming the fixture's live mux pin to another pin's name through the prompt renamed the tmux session (the fake tmux recorded it), then the store refused the duplicate.

- The prompt refuses with the edit form's name check (`pin_edit::check_name`, one function for both rename UIs): a blank name (`pin: nothing entered`), a name that is not a pin id, or another pin's name (``a pin named `x` already exists``), on its error line, and commits nothing.
- The store write checks the name again (`SlotStore::check_rename`) before it renames a live mux's session or sets the name override, for a name taken after the prompt opened. The executor's rename runs over a given store (`rename_slot_with`) so tests can drive it.
- Tests: `executor_rename_tests.rs` (prompt to a taken name changes nothing: no tmux rename, no override, no store write; the executor alone refuses a taken name; a free name still renames session, pin, and override), a `check_name` test (blank, invalid id, taken, own, free), a `check_rename` test, and the text-entry consistency check for both UIs.
- Docs: operations guide, changelog, ADR 0170 status, operator-review row. Follow-up CSP-799: the CLI's `pin rename` has the same order.

**Regressions:** none.

**Consistency wins:**
- The Rename… prompt refuses a taken or unusable pin name in the prompt with the form's reason, instead of closing, renaming the live session, and failing in the store (frames `prompt-taken-*`).
- The edit form's blank pin name reads `pin: nothing entered`, the prompt's reason, instead of the store's id rule with an empty pair of backticks (frames `form-blank-*`).

Checked with `conspectus tui --snapshot` at 100x30 and 50x16 (`/tmp/tuifw-baseline/frames-778-*`); the other captured frames are byte-identical. Full isolated suite 4799 passed, 0 failed, 1 ignored (one rerun for the known `discovery::forge` timing flake); fmt, both clippy runs, doc, examples, and `git diff --check` pass.
<!-- SECTION:FINAL_SUMMARY:END -->
