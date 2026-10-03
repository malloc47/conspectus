---
id: CSP-431
title: Adopt `tui-skeleton` for background-load placeholders
status: Done
assignee: []
created_date: '2026-06-19 00:59'
labels:
  - h-widg
milestone: m-11
dependencies:
  - CSP-184
ordinal: 370000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- **Landed 2026-07-06 (`0d1bc63`) as an in-tree substrate**,
  deviating from the literal `tui-skeleton` scope. See commit
  message for the full rationale; short version:
    - The story's row-panel / detail-panel skeleton scope had
      two frictions: (a) adding a crate requires an ADR per
      ADR 0024 for a ~1s initial-load window, and (b)
      replacing the operator's visible rows with skeleton
      shapes every 30s refresh would regress UX.
    - Operator's forward-looking need — 10s+ forge fetches and
      transcript loads on selected nodes — is better served
      by a general "in-flight async op" substrate visible in
      the status bar, not by skeletons over the main panels.
- What landed:
    - `InFlightKind` + `InFlightOp` model + `App::in_flight_ops`
      storage.
    - `Msg::InFlightStart { kind, label }` and
      `Msg::InFlightFinish(kind)` reducer arms.
    - Runtime dispatches these around all three discovery-spawn
      sites via a new `spawn_tracked_discovery` helper.
    - Status-bar chip: Braille spinner glyph
      (`⠋⠙⠹⠸⠼⠴⠦⠧`, ~120ms/frame) + label, styled
      `panel_focus_accent + BOLD`.
    - Reducer test coverage grows 28 → 32; UI tests pin
      chip presence + hide-after-finish.
- Follow-on when new async surfaces land: extend `InFlightKind`
  with variants like `ForgeFetch(NodeId)` /
  `TranscriptLoad(NodeId)` and dispatch a start/finish pair
  around each spawn.
- Blockers: `CSP-184` (retroactively landed).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WIDG-007`
