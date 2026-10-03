---
id: CSP-167
title: Add non-blocking graph refresh data adapter
status: To Do
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies:
  - CSP-165
  - CSP-166
ordinal: 394000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: implement a data adapter that runs initial discovery, feeds the
  resolved graph into the app state, and refreshes on `r` or the
  configured interval without blocking input. Preserve provider
  diagnostics and stale/error status in the UI. Keep the adapter shaped
  so Phase 7 server snapshots can replace in-process discovery later.
- Tests: unit/integration tests with fake discovery covering initial
  load, refresh success, refresh failure preserving prior graph, refresh
  replacing the selected row while retaining selection, and disabled or
  long refresh intervals.
- Manual checks: run `cargo run -- tui`, change local graph inputs, press
  `r`, and verify rows update without losing usable terminal state.
- Blockers: `CSP-165`, `CSP-166`.
- **v1 slice landed**: synchronous initial discovery + `r`
  refresh wired in the runtime. Discovery runs on the main
  thread, briefly blocking input during the call. Selection
  retention across refresh comes from the existing reducer
  (`CSP-165`). Remaining work (filed as follow-on):
  - `CSP-184`: move discovery onto a background thread with
    mpsc back-channel so input never blocks; add timer-driven
    auto-refresh on the configured `refresh_interval`;
    preserve provider diagnostics for the status-bar chips;
    shape so a Phase 7 server snapshot transport can swap in
    without UI changes.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P8-008`
