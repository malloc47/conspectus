---
id: CSP-288
title: Migrate the TUI detail pane to SQLite
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-281
  - CSP-287
ordinal: 481000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace the snapshot walks in `src/tui/detail.rs` with
  `Connection`-driven queries. The detail pane sections from
  ADR 0033 stay; only the data source changes.
- Tests: parity with the existing detail-pane snapshots; runtime
  smoke test confirms the pane still re-renders on selection
  changes.
- Blockers: `CSP-281`, `CSP-287` (so the typed-row patterns are
  settled before the TUI consumes them).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
New
`build_node_detail_from_conn(conn, target, home) -> Result<Option<NodeDetail>>`
is the SQLite consumer surface. It calls
`query::read_snapshot` per-call and runs the existing typed-Rust
view-model assembly. `build_node_detail` survives for
fixture-heavy tests and producer-side callers that still start
from a typed snapshot.
Trade-off vs. per-section SQL (which CSP-287 used): the detail
builder's view-model assembly (kind-dispatched header fields,
mux/pr/lineage subqueries with ambiguity counts, link summaries
with shortened paths) is complex enough that rewriting each
helper as SQL doubles the line count for no observable
behavior change. Routing through `read_snapshot` keeps the
assembly in one place and still satisfies the consumer-side
contract — the function takes a `Connection`, returns a typed
view-model, and never persists a `GraphSnapshot`. A new parity
test asserts the two entry points produce equal `NodeDetail`s
for the same input, catching drift if a future story refactors
only one path. Documented the choice in the module doc as a
pattern: per-section SQL when per-cell formatting is trivial
(projections, node show); `read_snapshot` bridge when typed
assembly is complex (TUI detail pane).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-010`
