---
id: CSP-499
title: Move scroll reconciliation into the reducer
status: Done
assignee: []
created_date: '2026-07-02 02:16'
labels:
  - h-tui
milestone: m-11
dependencies: []
ordinal: 108000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed across waves 1 + 2 on 2026-07-02..07-03.
  Wave 1 dropped `Cell` interior mutability (fields become
  plain `u16` / `Option<usize>`; `ui::draw` takes `&mut App`;
  `adjust_*_scroll` become `&mut self`). Wave 2 introduced
  `Msg::LeftViewportChanged { viewport_height }` and
  `Msg::ExplorerViewportChanged { cursor_first_row,
  cursor_last_row, viewport_height }`; the reducer arms
  call the existing adjust math. Draw dispatches the Msgs
  mid-frame and reads `left_scroll()` / `explorer_scroll()`
  as pure getters. `draw` stays `&mut App` because the two
  viewport-Msg dispatches happen inside the draw path (the
  outer draw call frame's inner heights are the smallest
  boundary that has the layout inputs). A strict `&App →
  buffer` shape would require restructuring draw into
  separate measure + render passes — recorded in the `draw`
  docstring as optional ADR 0085 contract-5 cleanup, not a
  correctness need.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TUI-005`
