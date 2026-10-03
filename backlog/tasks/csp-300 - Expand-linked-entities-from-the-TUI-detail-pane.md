---
id: CSP-300
title: Expand linked entities from the TUI detail pane
status: Done
assignee: []
created_date: '2026-05-30 04:47'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 416000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a right-pane keybinding that expands linked entities in
  place. For a selected agent session, the Mux section's linked
  `tmux:<name>` row should expand into the mux's full detail fields.
  For a selected mux session, the Session section's linked
  `harness:<session_key>` rows should expand into each agent
  session's full detail fields. Keep the compact linked rows by
  default so the right pane remains scannable.
- Tests: reducer/keymap coverage for the new keybinding, detail
  renderer tests for collapsed versus expanded linked entities, and
  at least one mux-with-two-sessions case.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The detail view-model now attaches one-level target
details to linked mux/session summary rows. `e` toggles linked
details from either pane, and `Enter` does the same when the
right pane has focus. The expanded rows stay nested under the
existing Mux/Session section instead of changing the left-tree
selection.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-026`
