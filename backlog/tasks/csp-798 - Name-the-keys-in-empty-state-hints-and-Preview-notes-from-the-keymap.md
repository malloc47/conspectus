---
id: CSP-798
title: Name the keys in empty-state hints and Preview notes from the keymap
status: To Do
assignee: []
created_date: '2026-10-30 22:23'
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
- [ ] #1 Every key the left pane's empty states and the Preview's notes name comes from the keymap binding that takes it
- [ ] #2 The status scan covers those files and passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
