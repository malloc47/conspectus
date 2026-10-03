---
id: CSP-266
title: >-
  Add TUI interaction regression tests for row expansion, scrolling, and attach
  resolution
status: Done
assignee: []
created_date: '2026-05-24 20:27'
labels:
  - test
milestone: m-11
dependencies:
  - CSP-262
  - CSP-183
ordinal: 277000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: build a thin test driver around `App` that applies fixed
  key/action sequences at deterministic terminal sizes. Cover the
  cases that escaped pure row snapshots: expanded ambiguous mux
  candidates remain navigable, selection stays visible while
  groups expand/collapse, attach target resolution never targets
  the current tmux session, preview/detail panes tolerate missing
  or ignored links, and the row cursor can move past duplicate or
  overridden candidate rows.
- Tests: reducer/action tests plus Ratatui buffer snapshots for the
  smallest useful set of fixed viewports. Prefer structured
  assertions for navigation state and snapshots only where layout
  regressions are the risk.
- Manual checks: run `conspectus tui` against a replayed or live
  ambiguous-mux fixture only when adding a new interaction failure.
- Blockers: `CSP-262`; coordinates with `CSP-183` so buffer
  snapshot coverage is not duplicated.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added scenario-backed `App` reducer/action tests using
the named `CSP-306` worlds. Coverage now asserts ambiguous mux
candidate rows remain navigable after expansion, selection snaps
to a visible row when a refresh removes the selected row, and
attach target resolution refuses the tmux session hosting the
current TUI.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `TEST-005`
