---
id: CSP-268
title: >-
  Detect session live status (running / waiting / idle / error) and surface it
  as a row glyph and per-status header chip
status: To Do
assignee: []
created_date: '2026-05-25 02:00'
updated_date: '2026-10-31 08:15'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-185
ordinal: 460000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: this is the agent-deck signal the styling overhaul
  deliberately did not invent (color buckets stand in for v1).
  Real status detection wants a data-layer feature: observe mux
  pane changes (delta against last capture; tied into the throttle
  work in `CSP-185`), join hook-sidecar evidence (ADR 0028) when
  the harness exposes a "waiting on permission" or "tool error"
  state, and expose a new `SessionStatus` enum on the row
  view-model. The renderer reads it through the existing `Theme`
  additions (`status_running`, `status_waiting`, `status_idle`,
  `status_error` — already reserved in the badge widget's color
  vocabulary). Header chips swap their per-mux-state breakdown
  for per-status counts when the operator opts in via
  `[tui.show_status]`.
- Tests: pane-delta detector unit tests, hook-sidecar status
  extraction tests, reducer tests for the new `Msg::SessionStatus`,
  Ratatui snapshots for the colored row glyph and header chip
  variants.
- Blockers: needs its own ADR (decision: where status lives in the
  graph; whether it's a candidate-link relation or a property on
  `AgentSessionNode`; cadence + cost of pane-delta polling). Lives
  downstream of `CSP-185` so the throttle/freshen work pays for the
  extra capture cadence.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-022`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): no `status_running`, `status_waiting`, `status_idle`, or `status_error` colors exist or are reserved. New colors are host roles registered with the framework theme (`ROLES` in `src/tui/theme.rs`, ADR 0153), set under `[tui.theme]`; a header chip is a `tuikit::chip::StatusChip` (ADR 0157); a row glyph is a cell of the view's column schema built in `src/tui/rows/` (ADR 0183), and a new glyph needs a vetted entry in the glyph table (`tuikit::glyphs` or the host's `src/tui/icons.rs`, ADR 0154).
<!-- SECTION:NOTES:END -->
