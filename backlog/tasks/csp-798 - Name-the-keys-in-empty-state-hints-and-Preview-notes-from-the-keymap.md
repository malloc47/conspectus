---
id: CSP-798
title: Name the keys in empty-state hints and Preview notes from the keymap
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 22:23'
updated_date: '2026-10-30 23:58'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-796
priority: low
ordinal: 884000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-796 named the keys in status and refusal messages from the keymap (`keybindings::key_name`) and extended the status scan of `tests/overlay_keymap_hygiene.rs` to `app.rs`, `app/`, and `actions.rs`, with a rule that also flags a key named in prose (a key before a colon, a single character in backticks, "press <key>"). Messages outside those files still name keys by hand and can drift from the keymap: the left pane's empty-state hints in `src/tui/ui/left_panel.rs` (``Press `s` (or Ctrl-/) to search…``, ``Press `F` to clear filters, `f` to edit.``, ``Press `.` for the menu (Clean up…).``, ``Press `Ctrl-R` to ask the forges again or `q` to quit.``, ``Press `r` to refresh or `q` to quit.``, ``Clear `Unpinned only` in Controls (`f`)…``) and the Preview's notes in `src/tui/ui/preview_pane.rs` (``pin unavailable — try `r` to refresh``, ``tmux target not found — try `r` to refresh``).

Scope: name each key from its binding with `keybindings::key_name`, and add `src/tui/ui` (or the two files) to the status scan's trees so the prose rule holds them. The help page's annotations (`src/tui/widgets/help.rs`) describe overlays' keys in prose by design (ADR 0192) and stay out; `runtime/launch.rs`'s "press Enter to return" is the shell's prompt after a viewer exits, not a keymap key.

TUI changes: none intended (the labels are the ones the hints write); list any in the summary.

Tests: the extended scan passes; each hint's test reads its key from the keymap.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every key the left pane's empty states and the Preview's notes name comes from the keymap binding that takes it
- [x] #2 The status scan covers those files and passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Extend the status scan of `tests/overlay_keymap_hygiene.rs` to all of `src/tui/ui` (replacing its `ui/header.rs` entry); it flags exactly the story's twelve strings. Widen its prose rule to keys in backticks (`` `Ctrl-R` ``, which only the `q` beside it made the scan catch) and to hint segments a double space separates (the Search header's hand-written `< older  > newer`).
2. Add `keybindings::key_names(id)` (every key of a binding, first is `key_name`'s) for the search empty state's "(or Ctrl-/)", the second key of `search.contents`.
3. Name each key of the left pane's empty states, the Search header's paging hint, and the Preview's two notes from its binding (`search.contents`, `search.older`, `search.newer`, `filters.clear`, `controls`, `row.menu`, `refresh.hard`, `refresh`, `quit`); wording unchanged.
4. Tests: drive each empty state and note and compare it with the text built from `key_name`; pin the labels in `a_message_names_the_key_its_binding_takes` so no text changed; extend the scan's self-tests.
5. Check frames with `tui --snapshot` (empty views), and run fmt, both clippy runs, `just tuikit-features`, the isolated suite, doc, examples, diff check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
With `src/tui/ui` added to STATUS_TREES and before any change, the status scan flagged exactly the story's twelve strings (ten in left_panel.rs, two in preview_pane.rs) and nothing else in ui/; the Search header's `< older  > newer` was not flagged, so the rule now also splits hint segments at a double space, and flags keys in backticks (`Ctrl-R`).
Frames (`tui --snapshot --snapshot-pane left` over an empty fixture, views 1/3/5/6, widths 120 and 50) read exactly the old text: "Press `r` to refresh or `q` to quit.", "Press `Ctrl-R` to ask the forges again or `q` to quit.", "Press `.` for the menu (Clean up…).", "Press `s` (or Ctrl-/) to search the contents of sessions and mux panes." (frames in /tmp/tuifw-baseline/frames-798-*.txt).
Checks: fmt; clippy --workspace --all-targets --all-features -D warnings; `just tuikit-features`; full isolated suite 4793 passed, 0 failed, 1 ignored; rustdoc -D warnings; cargo build --examples; independence guard; git diff --check.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Every key the left pane's empty states and the Preview's notes name now comes from the keymap binding that takes it (commits a0defc6d, c3bae1e2).

Converted, wording unchanged (no string named a wrong key, so none was corrected):
- `src/tui/ui/left_panel.rs` empty states: "Press `s` (or Ctrl-/) to search…" (`search.contents`, first and second key), "Press `s` to search again, `<` for an earlier search." (`search.contents`, `search.older`), "Sessions that match appear as they are found. Press `F` to clear.", "Press `F` to clear.", "Press `s` to search again, `F` to clear." (`filters.clear`), "Press `F` to clear filters, `f` to edit." (`controls`), "Clear `Unpinned only` in Controls (`f`)…", "Press `.` for the menu (Clean up…)." (`row.menu`), "Press `Ctrl-R` to ask the forges again or `q` to quit." (`refresh.hard`, `quit`), "Press `r` to refresh or `q` to quit." (`refresh`).
- `src/tui/ui/preview_pane.rs`: "pin unavailable — try `r` to refresh", "tmux target not found — try `r` to refresh" (`refresh`).
- Beyond the list: the Search view header's paging hint `< older  > newer` (`search.older`, `search.newer`).

`keybindings::key_names(id)` (every key of a binding, in order) was added for the second key in "(or Ctrl-/)", since `key_name` gives only the first.

Scan: the status scan of `tests/overlay_keymap_hygiene.rs` covers all of `src/tui/ui` (replacing its `ui/header.rs` entry). It also flags keys in backticks (`` `Ctrl-R` ``) and hint segments split by a double space, so the `< older  > newer` shape is caught; nothing else in the scanned trees trips the wider rule. Before the conversion it flagged exactly the twelve listed strings. ADR 0191's status, the rule-to-crate table in `tests/support/source_tree.rs`, and operator-review.md record it.

Tests: `src/tui/ui/hint_keys_tests.rs` drives each empty state (empty world, empty PRs, a filter that hides every row, empty History and pinned-only History, the Search view with no search and with a search that found nothing, a running, finished, and reading search on the lists), the Search header, and both Preview notes, building each expected text from `key_name`. `a_message_names_the_key_its_binding_takes` pins the eight new labels, and a new test pins `key_names("search.contents")` to `s`, `Ctrl-/`. The PRs view test reads `refresh.hard` from the keymap.

Checks: fmt; clippy --workspace --all-targets --all-features -D warnings; `just tuikit-features`; full isolated suite (4793 passed, 0 failed, 1 ignored); rustdoc -D warnings; cargo build --examples; git diff --check.

Regressions: none. `tui --snapshot` frames of the empty Sessions, PRs, History, and Search views (widths 120 and 50) read exactly the old text.
Consistency wins: none visible. These notes now follow the keymap when a key is rebound.
<!-- SECTION:FINAL_SUMMARY:END -->
