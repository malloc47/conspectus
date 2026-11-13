---
id: CSP-345
title: In-viewer navigation into forks / child sessions
status: To Do
assignee: []
created_date: '2026-06-03 00:58'
updated_date: '2026-10-31 08:15'
labels:
  - h-viewer-native
milestone: m-11
dependencies:
  - CSP-340
ordinal: 219000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: when the displayed transcript references a fork or
  a child session, expose a way to jump into that session's
  transcript without leaving the modal (e.g. `→` over a
  chip, or a per-fork chip with `Enter`). Needs lineage
  from the graph layer; the bridge gains
  `build_viewer_state_for_child(parent, child_session_id)`
  or similar. Stack-of-states inside the modal so
  Backspace returns to the previous transcript.
- Open questions: should the lineage walk go through
  conspectus's resolved graph (per ADR 0005 / ADR 0018) or
  through harness-specific intra-session lineage encoded in
  the transcripts themselves? The latter keeps the viewer
  extractable; the former is richer.
- Tests: bridge mapping for `parent_session` references in
  each harness; modal stack push/pop; lineage-not-found
  fallback.
- Blockers: `CSP-340`. Pairs naturally with
  `CSP-343` for chip-as-click-target affordance.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-VIEWER-NATIVE-013`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the viewer is the framework's `Pager` in Screen mode over the viewer's `Transcript<M>` content (`src/viewer/state.rs`, `src/viewer/widget.rs`, ADR 0180); `ViewerState` is gone. The bridge (`src/tui/viewer_bridge.rs`) builds the content, so a child transcript is a second content it builds; a stack is most simply a second viewer pushed on the host's modal stack, which `Esc` or `q` closes back to the parent (house rule 18). Taking lineage from the transcripts keeps `src/viewer` free of graph imports.
<!-- SECTION:NOTES:END -->
