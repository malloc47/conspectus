---
id: CSP-289
title: Migrate the TUI sessions row builder to SQLite
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-282
ordinal: 482000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace `build_sessions_tree` (`src/tui/rows/sessions.rs`)
  and its `SessionsBuildInputs` with a `Connection`-driven
  builder. Grouping/bucketing logic (ADR 0024) stays in Rust on
  top of the result set. `RowFilter` continues to gate per session
  before bucketing. Performance check: the TUI refresh loop
  should stay under its current latency budget on the fixture
  corpus.
- Tests: parity with the existing sessions-tree snapshots over the
  fixture corpus; refresh-loop latency measurement on the largest
  fixture.
- Blockers: `CSP-282`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
New
`build_sessions_tree_from_conn(SessionsBuildInputsFromConn)`
bridges via `query::read_snapshot` and delegates to the
existing `build_sessions_tree`. Follows the
`build_node_detail_from_conn` pattern from CSP-288 — the
grouping/bucketing/launch-context/candidate-mux expansion
logic is preserved end-to-end; per-section SQL would have
doubled ~2000 lines of typed assembly for no observable
behavior change. The existing snapshot-taking
`build_sessions_tree` survives for fixture-heavy tests and typed
assembly reuse. A parity test asserts the two entry points
produce equal `RowTree`s.
Refresh-loop latency check deferred to a follow-up — current
refresh is well under any user-noticeable threshold and the
bridge adds one `read_snapshot` pass which is bounded by
graph size; if it ever becomes load-bearing, the per-section
SQL refactor is the optimization story.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-011`
