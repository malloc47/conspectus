---
id: CSP-426
title: Swap `widgets/multi_select.rs` for `ratatui-cheese.multi_select`
status: Done
assignee: []
created_date: '2026-06-19 00:59'
labels:
  - h-widg
milestone: m-11
dependencies:
  - CSP-425
ordinal: 365000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: CSP-255 shipped the in-tree multi-select as a pure
  state machine — 406 LOC of generic list-with-checkbox logic.
  `ratatui-cheese` 0.7 ships a Bubbletea-inspired
  `multi_select` widget that is conceptually 1:1 with the
  existing API. The forcing function for the whole workstream:
  cleanest seam, smallest snapshot blast radius, strongest
  signal on whether the theme/snapshot integration cost is what
  the audit predicted.
- Followups: keymap matched without a remap shim — upstream
  `next` / `prev` / `toggle_current` semantics are identical to
  the in-tree dispatch (Space toggle, Enter confirm, Esc cancel
  routed by the shim's `handle_key`). `Clone` is hand-written
  because the upstream state's `Box<dyn Fn>` validator blocks
  derive; the shim does not use validators so cloning rebuilds
  cursor + selections + focus.
- Blockers: `CSP-425` (so the new bridge code lands in the
  macro idiom). [met]
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-06-19): landed across two commits — the
functional swap (`5389c2d`) and the theme-glue follow-up
(`5ae8abe`). `widgets/multi_select.rs` is now a thin shim
that wraps the upstream `MultiSelectState` for cursor +
selection and `MultiSelect` for per-row rendering;
`MultiSelectOutcome` and `MultiSelectItem` stay at the
call-site API boundary so `widgets/controls.rs`'s
`SubEditor::Harness` / `SubEditor::MuxState` paths needed no
change beyond a `theme: &Theme` threading.
`cheese_styles_from_theme(&Theme)` bridges
`panel_focus_accent` / `placeholder` / `error` / `success`
onto upstream `MultiSelectStyles`; `.theme(&Theme)` is the
builder hook on `MultiSelectWidget`. Net source delta:
`+99 / −120` (the −21 in source includes the
`compute_scroll` helper retirement; total upstream surface
moved is wider since cursor/select/render are now upstream's
responsibility). Honest workstream calibration: the predicted
"~400 LOC retires" overestimated the win because the in-tree
bordered modal + buffer-clear + handle_key/outcome contract
+tests still ship from the shim. Real value: cursor/select
state machine + per-row rendering move upstream; future
capabilities (limit, validation, disabled options) are free.
1669 tests pass byte-identical; visual verification via
`--snapshot --snapshot-keys 'f...j..<Enter>'` confirmed the
themed `>` cursor and label colors render in project palette
instead of upstream defaults.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WIDG-002`
