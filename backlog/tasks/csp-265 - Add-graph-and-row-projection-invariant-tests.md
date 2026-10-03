---
id: CSP-265
title: Add graph and row-projection invariant tests
status: Done
assignee: []
created_date: '2026-05-24 20:27'
labels:
  - test
milestone: m-11
dependencies:
  - CSP-262
ordinal: 276000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add table-driven and, where practical, property-style
  tests for invariants that cut across specific scenarios: ignored
  candidates never resolve as active relationships; stronger
  current-session evidence beats launch evidence; hook-sidecar
  records produce at most one active link per `(mux, pane_id)` key;
  TUI session rows dedupe mux indicators by target; unresolved or
  ignored candidates remain diagnosable without becoming preferred
  rows.
- Tests: invariant tests over hand-built graph fragments plus a
  small matrix of replay fixtures. Add `proptest` only if a
  bounded generator demonstrates value; otherwise keep the first
  slice deterministic and table-driven.
- Manual checks: none.
- Blockers: none for table-driven invariants; `CSP-262` before
  running invariants against replay fixtures.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added four deterministic invariant tests to
`tests/testing_replay.rs`: ignored mux candidates remain visible
as evidence but never resolve, ambiguous TUI session rows dedupe
candidate rows by mux target, replay worlds enforce at most one
active hook-sidecar link per `(mux, pane_id)`, and stronger
current-session fd evidence wins over stale launch history.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `TEST-004`
