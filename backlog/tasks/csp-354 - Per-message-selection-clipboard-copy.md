---
id: CSP-354
title: Per-message selection + clipboard copy
status: To Do
assignee: []
created_date: '2026-06-03 13:09'
labels:
  - h-viewer-native
milestone: m-11
dependencies:
  - CSP-340
ordinal: 220000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: introduce per-message selection inside the viewer
  modal. `J`/`K` (capital) move the selection forward/back
  one turn. The selected turn shows a highlighted bar in
  the gutter's far-left column (over the chip pill or
  alongside it). A keybind (`yy` like vim's yank, or `Ctrl-Y`)
  copies the selected turn's body to the system clipboard.
- State: `ViewerState` gains `selected_turn: Option<usize>`
  (index into the visible turns produced by build_body_lines).
  Reducer messages: `SelectPrevTurn`, `SelectNextTurn`,
  `ClearSelection`, `CopySelectedBody`.
- Renderer: when `selected_turn == Some(i)`, the chip line
  (and continuation lines) of turn `i` paint the leftmost
  cell of the gutter as a highlight bar (e.g. `▌` in the
  chip color, BOLD).
- Clipboard: route through a small abstraction (already
  available in the wider conspectus surface via `arboard` /
  the existing `clipaste` integration), or via OSC 52 for
  SSH-friendly copy. ADR check on the dep before adding to
  the viewer's allow-list.
- Tests: reducer unit tests for selection cycling; widget
  snapshot test for the highlight bar; clipboard call
  behind a `BinaryProbe`-style seam so unit tests don't
  actually touch the host clipboard.
- Blockers: `CSP-340`. Pairs naturally with
  `CSP-344` (mouse selection) and the
  chunk-loading story so chunked transcripts have stable
  turn indices.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-VIEWER-NATIVE-015`
