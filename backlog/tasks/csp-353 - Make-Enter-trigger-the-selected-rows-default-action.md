---
id: CSP-353
title: Make Enter trigger the selected row's default action
status: Done
assignee: []
created_date: '2026-06-03 12:44'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 438000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: existing dispatcher coverage in
  `selected_default_action_tests`
  (`empty_selection_falls_back_to_toggle_expand`,
  `group_row_resolves_to_toggle_expand`,
  `unmuxed_session_resolves_to_view`,
  `muxed_session_resolves_to_attach`,
  `mux_candidate_child_resolves_to_attach`,
  `mux_view_mux_row_resolves_to_attach`,
  `unbound_pin_row_resolves_to_launch_pin`) plus new
  `unmuxed_session_resolves_to_view_regardless_of_viewer_support`,
  the `remap_for_focus_*` focus-specific suite proving right-pane
  `Enter` becomes `ExplorerEnter`, and a status-hint test for the
  disabled-attach fallback
  (`contextual_status_surfaces_disabled_attach_reason_for_current_tmux_session`).
- Manual checks: validated with the existing fixture suite and the
  `--snapshot` harness; round-trip attach (`CSP-194`) and viewer
  launch (`CSP-340`) remain the operator-facing
  verification paths.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Closed. `Enter` on the left pane dispatches via
`selected_default_action` (`src/tui/runtime.rs`): mux rows and
muxed/ambiguous agent sessions attach (reusing `attach_action`);
un-muxed agent sessions open the native transcript viewer
(reusing `view_action`); pin rows launch; group rows
expand/collapse. Right-pane `Enter` is remapped to
`ExplorerEnter` by `remap_for_focus`, leaving the detail
explorer's drill/expand/Node-zone-copy semantics untouched. As
companions, `v` works on mux rows (opens the preferred linked
session's transcript) and vi-style `h`/`l` fold bindings drive
explicit expand/collapse now that `Enter` is no longer the
universal toggle. Status hints advertise the right verb per
row kind via `default_action_status_hint`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `T8-043`
