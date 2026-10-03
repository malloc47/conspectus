---
id: CSP-360
title: Gate left-pane tree navigation keys on left-pane focus
status: Done
assignee: []
created_date: '2026-06-04 15:59'
labels:
  - h-obs
milestone: m-11
dependencies: []
ordinal: 117000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`remap_for_focus()` in `src/tui/runtime.rs` now drops
`Msg::ExpandRow` / `Msg::CollapseRow` (h/l/Left/Right) on
right-pane focus so they no longer mutate the left tree the
operator isn't driving, and remaps `Msg::Home` / `Msg::End`
(g/G/Home/End) to new `Msg::ExplorerHome` / `Msg::ExplorerEnd`
variants that snap the explorer cursor to its first / last
row — the right-pane-equivalent the operator expects. The
reducer dispatches the new variants through a small
`explorer_jump_cursor_to(usize)` helper that clamps to the
current row count and mirrors `move_selection_to` for the
left tree. `Msg::CycleFocus` (Tab) is intentionally left
alone since it's the focus toggle itself. Tests:
`remap_for_focus_right_suppresses_left_tree_expand_collapse_keys`
pins the h/l drop;
`remap_for_focus_right_routes_home_and_end_into_the_explorer`
pins the g/G remap and that Tab stays the focus toggle;
`explorer_home_and_end_snap_cursor_to_first_and_last_row`
pins the reducer dispatch. Followups (out of scope): PageDown
/ PageUp currently remap to a single-row explorer step
rather than a true page — the per-frame viewport height
isn't threaded through, and that's worth its own story if
operators ask for it.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-OBS-007`
