---
id: CSP-346
title: Lazy / chunk-by-chunk transcript loading around compaction boundaries
status: To Do
assignee: []
created_date: '2026-06-03 12:44'
updated_date: '2026-10-31 08:15'
labels:
  - h-viewer-native
milestone: m-11
dependencies:
  - CSP-343
ordinal: 223000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: NATIVE-011's render cache makes scroll-only frames
  O(1), but the *first* draw still composes every visible
  turn through `tui_markdown::from_str`. Very large Claude
  sessions (thousands of turns, many tool blocks) take a
  visible beat to open the viewer. Operator suggestion:
  load chunks lazily, anchored at `compact_boundary` /
  `compacted` records — initial open renders just the
  most-recent compaction window, and earlier chunks load
  on demand as the operator scrolls past the chunk's top.
- Implementation outline: introduce a `ChunkedTranscript`
  in `src/viewer/model.rs` (or a `ChunkBoundary` on
  `TranscriptDocument`) carrying the compaction-aware
  spans of the source. Parsers split at boundaries.
  Widget keeps the active chunk in `ViewerState`; reducer
  handles "scroll past chunk top" by loading the previous
  chunk. Cache key gains the chunk identity.
- Open questions: how to render the chunk seam (single
  rule with a "load previous" affordance vs. eager fetch
  on approach); whether to also chunk on time-of-day
  boundaries for sessions without explicit compaction;
  interaction with search (`CSP-339`), which
  needs to span chunks.
- Tests: parser-side fixture covering multi-chunk
  boundaries; widget chunk-load reducer test; performance
  smoke against a real session.
- Blockers: `CSP-343` (the render cache and
  the gutter layout are prerequisites).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-VIEWER-NATIVE-014`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): `ViewerState` is gone; the viewer's state is the `Transcript<M>` content (`src/viewer/state.rs`, ADR 0180) inside the framework's `Pager`, and Markdown turns render through `tuikit::richtext::markdown` (`src/viewer/render.rs`, ADR 0181). A chunk seam and load-on-scroll would live in the content (`PagerContent`); check how the pager keeps its position when the content's line count changes under it.
<!-- SECTION:NOTES:END -->
