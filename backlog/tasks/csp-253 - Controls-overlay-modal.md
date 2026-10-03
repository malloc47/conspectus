---
id: CSP-253
title: Controls overlay (modal)
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies:
  - CSP-250
  - CSP-251
  - CSP-255
ordinal: 449000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: render a centered modal with sections for View,
  Grouping (scoped to active view), Filters (scoped to active
  view), and Sort (global). Arrow-key + Enter navigation, Esc
  backs out one level, mouse click support inside the overlay.
  Inline accelerator hints (`[1]`, `[2]`, …) per row. Drill-in
  sub-editors: harness multi-select, max-age text input (reuses
  ADR 0030 primitive), mux-state multi-select. Filter chips
  render in the status bar via `CSP-256`.
- Tests: snapshot tests for overlay open, each sub-editor open,
  chip applied state, cleared state. Reducer tests for arrow-key
  navigation skipping section headers and for sub-editor
  Confirm/Cancel outcomes.
- Blockers: `CSP-250`, `CSP-251`, `CSP-255`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in `src/tui/widgets/controls.rs` —
`ControlsOverlayState` renders the View/Grouping/Filters/Sort
sections with arrow-key + Enter navigation, Esc back-out, and
inline accelerator hints. Drill-in sub-editors cover
harness multi-select, max-age text input, and mux-state
multi-select. Snapshot + reducer tests guard each surface.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-004`
