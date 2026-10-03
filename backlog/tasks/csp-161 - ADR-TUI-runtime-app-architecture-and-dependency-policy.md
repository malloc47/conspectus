---
id: CSP-161
title: 'ADR: TUI runtime, app architecture, and dependency policy'
status: Done
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies: []
ordinal: 388000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Recorded as ADR 0024. Ratatui + crossterm with an
in-tree Elm-style app loop; pure reducer and view-models;
`std::thread::spawn` + `mpsc` for background work (no async
runtime in v1); buffer-snapshot tests via `insta`. Dependency
policy narrows what later TUI work can pull in without a
follow-on ADR.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-002`
