---
id: CSP-344
title: Mouse bindings inside the viewer
status: To Do
assignee: []
created_date: '2026-06-03 00:58'
labels:
  - h-viewer-native
milestone: m-11
dependencies:
  - CSP-343
ordinal: 218000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: scroll-wheel events translate to ScrollUp /
  ScrollDown; click positions the cursor / selects a turn
  boundary. crossterm mouse events are already enabled
  elsewhere in the TUI; the modal just needs a handler
  branch in `handle_viewer_overlay_key` (or a new
  `handle_viewer_overlay_mouse`).
- Tests: mouse-event smoke through the reducer.
- Blockers: `CSP-343` (styling) so click
  targets land on visually-meaningful elements.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-VIEWER-NATIVE-012`
