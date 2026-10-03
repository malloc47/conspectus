---
id: CSP-458
title: Post-create/adopt focus and toast behavior
status: Done
assignee: []
created_date: '2026-06-24 20:11'
labels:
  - h-pin-tui
milestone: m-11
dependencies:
  - CSP-453
ordinal: 326000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: after a successful create or adopt, refresh the graph,
  expand the synthetic Pins group in the current view when present,
  select the new pin row, and show a toast/status message that
  distinguishes "pin created, not started" from "pin adopted; mux
  already running". Preserve this behavior across sessions, mux,
  and union views where pin rows or pinned sessions can appear.
- Tests: reducer/runtime tests for create success, adopt success,
  current-view row selection, Pins-group expansion, launch-state
  wording, and failure paths that leave the prior selection intact.
  Snapshot tests for the post-create toast and selected pin row.
- Blockers: `CSP-453`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Successful TUI create/adopt writes now refresh the graph,
expand the current view's synthetic Pins group when present, and
select the row that represents the affected pin. Grouped sessions
and mux views prefer the row inside Pins; flat/union views fall
back to the visible pinned entity row. The runtime posts a toast
that distinguishes `pin created, not started` from
`pin adopted; mux already running`, while the status line keeps
the existing store/write detail and reports when filters prevent a
visible row match.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-007`
