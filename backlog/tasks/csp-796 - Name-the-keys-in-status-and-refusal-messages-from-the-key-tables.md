---
id: CSP-796
title: Name the keys in status and refusal messages from the key tables
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 15:58'
updated_date: '2026-10-30 22:38'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-760
priority: low
ordinal: 882000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-759 generated the status bar's hints from the key tables and holds the status-line code (`runtime.rs`, `runtime/executor.rs`, `app/overlays.rs`, `ui/header.rs`) to a stricter hygiene rule, but messages elsewhere still name keys by hand, so they can drift from the keymap: `app/explorer_nav.rs` (`explorer: no drill history — press Backspace again to return to the left pane`, `Backspace: no reveal to return from · Tab to the right pane to back out of its drill`), `app/content.rs` (`Ctrl-F filters by the search the Search view shows (6)`), `runtime.rs` (`i: select an agent or mux session row to copy its id`), `actions.rs` (`pin_status_hint`'s `· Enter runs `conspectus rebuild x``, shown in the Preview; `attach_disabled_why`'s ``try `r` to refresh``).

Scope: name each key from the table that binds it (the scoped keymap of CSP-760, `RIGHT_PANE_KEYS`, `MAIN_VIEW_KEYS` until it merges), with `keys::key_label`, and extend the status scan of `tests/overlay_keymap_hygiene.rs` to `app.rs`, `app/`, and `actions.rs`.

TUI changes: none intended beyond key labels written in the one notation (`Ctrl-F` stays, `Backspace` stays); list any in the summary.

Tests: the extended scan passes; the messages' tests read the label from the table.

Blockers: CSP-760 (the scoped keymap gives every program-wide key a table row).
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `keybindings::key_name(id)`: the first key of the keymap binding `id` in `keys::key_label`'s notation (the key a hint names), panicking on an unknown id; the right pane's keys are keymap bindings too (`explorer.back`), so one lookup serves both tables.
2. Name each key from its binding: `explorer_nav.rs` (the armed Back: `explorer.back`; the left pane's Backspace: `reveal.back` and `focus.cycle`; the no-explorer `o`: `value.inspect`), `content.rs` (`search.keep`, `view.search`), `runtime.rs` (`row.copy-id`), `actions.rs` (`pin_status_hint`'s and the Rebuild entry's Enter: `row.default`; `attach_disabled_why`'s `r`: `refresh`). Text unchanged: the labels are the ones the messages wrote.
3. Extend the status scan in `tests/overlay_keymap_hygiene.rs` to `src/tui/app.rs`, `src/tui/app/` (replacing its `overlays.rs` entry), and `src/tui/actions.rs`; fix anything it then flags; update the scan table in `tests/support/source_tree.rs`.
4. Tests read the label from the table: update the existing assertions (Backspace, the pin hints) and add tests for messages that had none (the Ctrl-F toast, `i`, the refresh hint, `o` with no explorer, the Rebuild entry's detail); a keybindings test checks every id a message names is bound.
5. Snapshot one message per surface at 100x30 and 60x20 to confirm identical text; full checks.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented in 51779939. `keybindings::key_name(id)` (first key of a keymap binding, `keys::key_label` notation; panics on an unknown id). Messages now naming keys from bindings: `explorer.back` (armed Back), `reveal.back` + `focus.cycle` (left-pane Backspace), `value.inspect` (no-explorer `o`, beyond the story's list), `search.keep` + `view.search` (Ctrl-F outside the Search view), `row.copy-id` (`i`), `row.default` (the pin hints' Enter and the Rebuild menu entry's "(Enter also attaches)", beyond the list), `refresh` (missing mux node). Status scan widened to `app.rs`, `app/` (replacing its `overlays.rs` entry), `actions.rs`, plus a prose rule (`names_a_key_in_prose`): a key before a colon, a single backticked character, "press" before a named key; the old `i: select…` example moved from the prose list to the caught list. Tests: `a_message_names_the_key_its_binding_takes` pins every label (Backspace, Tab, Ctrl-F, 6, i, Enter, r, o), `a_message_cannot_name_a_key_the_keymap_does_not_bind`, exact-text tests for each message reading the key from the table (new: no-explorer `o`, `i`, Ctrl-F toast, refresh hint, Rebuild detail, the pin hints; updated: the Backspace assertions). Frames /tmp/tuifw-baseline/frames-796-after at 100x30 and 60x20 read as before. Out-of-scope same-class sites in `src/tui/ui/` filed as CSP-798. Checks: fmt, clippy -D warnings, full isolated suite 4774 passed / 0 failed / 1 ignored, cargo doc -D warnings, examples, git diff --check.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Status and refusal messages name their keys from the keymap: `keybindings::key_name(id)` returns the first key of a binding in the one notation, and the armed right-pane Back (`press Backspace again`), the left pane's `Backspace: no reveal to return from · Tab to the right pane…`, the no-explorer `` `o` opens long values``, `Ctrl-F filters by the search the Search view shows (6)`, `i: select an agent or mux session row…`, the pin hints' and the Rebuild menu entry's `Enter`, and ``try `r` to refresh`` take their keys from the bindings that bind them (the right pane's keys are keymap bindings, so one lookup covers `RIGHT_PANE_KEYS` too). The status scan in `tests/overlay_keymap_hygiene.rs` now covers `app.rs`, `app/`, and `actions.rs`, and also flags a key named in prose. ADR 0191's status and the scan table in `tests/support/source_tree.rs` record it; operator-review has the judgment calls. The same kind of hand-named keys remain in `src/tui/ui/` (the empty-state hints and the Preview's notes): filed as CSP-798.

Verification: every message has an exact-text test that reads its key from the table, `a_message_names_the_key_its_binding_takes` pins the labels the messages wrote before, and the widened scan passes (and catches the old strings, which its own test lists). `conspectus tui --snapshot` at 100x30 and 60x20 of the left-pane and right-pane Backspace messages and the Ctrl-F toast shows the same text as before. fmt, clippy, full isolated suite (4774 passed, 0 failed, 1 ignored), cargo doc, examples, and `git diff --check` pass.

**Regressions**
- None: no message's text changed.

**Consistency wins**
- None visible; the messages can no longer drift from the keys the keymap binds, and the scan holds the reducer and the actions to it.
<!-- SECTION:FINAL_SUMMARY:END -->
