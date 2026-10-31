---
id: CSP-344
title: Mouse bindings inside the viewer
status: To Do
assignee: []
created_date: '2026-06-03 00:58'
updated_date: '2026-10-31 08:15'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): re-scoped. Mouse capture is not enabled anywhere: the terminal guard (`src/tui/runtime/term.rs`, which `tests/terminal_hygiene.rs` keeps as the one place the terminal's modes change) turns on bracketed paste only. `handle_viewer_overlay_key` is gone; the viewer is the framework's `Pager` over `Transcript<M>` (ADR 0180), reached through the host's modal stack. The work is now: an ADR to turn mouse capture on (it takes over the terminal's own drag-to-select), set in the terminal guard; the runtime routing `Event::Mouse` to the top overlay; the wheel calling the pager's public scroll methods (`crates/tuikit/docs/controls/pager.md`, "the scroll calls are public for a host that routes one"); and a click, which needs the roadmap's Tier 2 mouse hit testing (an entry reporting the rect it drew, `crates/tuikit/docs/controls/overlay.md`, "Rendered areas") and the content mapping a row to a turn. As the first mouse story, it sets the pattern for every control.
<!-- SECTION:NOTES:END -->
