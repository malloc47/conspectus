---
id: CSP-282
title: Migrate the CLI agent projection to SQLite
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-280
  - CSP-281
ordinal: 475000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace `render_with(snapshot, Projection::Agent, opts)`'s
  code path with a `Connection`-driven implementation modeled on
  `src/output/agent_sqlite.rs` from the spike. All 17 cells
  covered (the spike's 9 plus `mux`, `mux-conf`, `pr`, `pr-conf`,
  `lineage`, `workspace`, `fork`, `declared`). Extend
  `v_sessions_with_repo` (or add sibling views) for the
  `pick_preferred`-mediated cells; lean on `resolved_relationships`
  so the resolver's tie-break stays authoritative. `RowFilter`
  sits on top of the result set — no SQL filter pushdown in v1.
- Tests: parity test that runs the old and new renderers from the
  same fixture and asserts byte-equal output across the existing
  `output::table` snapshot corpus. Width-aware truncation
  snapshots unchanged. Filter behavior preserved.
- Manual checks: `conspectus table --rows sessions` against a
  real `graph.sqlite`; visually compare to the prior output.
- Blockers: `CSP-280`, `CSP-281`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Production renderer at `src/output/agent.rs` covers
all 17 cells. `render_with(snapshot, Projection::Agent, opts)`
routes through `agent::build_agent_rows_from_snapshot` via the
new `query::materialize_snapshot` helper; the in-memory
`build_agent_rows` / `agent_cell` / `lineage_cell` /
`preferred_pr_for_session` / `session_*` helpers are deleted.
Cells assemble from one primary query (sessions joined to
`v_sessions_with_repo` and `aliases`) plus seven per-cell
side-lookups (`fetch_mux_lookup`, `fetch_branch_lookup`,
`fetch_lineage_lookup`, `fetch_workspace_lookup`,
`fetch_fork_lookup`, `fetch_declared_lookup`, plus a global
`fetch_global_pr`).
`RowFilter` runs on top of the result set; mux candidate count
feeds the filter's MuxStateKey input. The spike's
`src/output/agent_sqlite.rs` is removed.
Two latent CSP-280 bugs surfaced and were fixed here:
`branch_has_forge_pr` saved view (`v_pr_by_branch`) had the
direction wrong — production discovery and the in-memory
`preferred_pr_for_session` both build the link source=ForgePr,
target=Branch (despite the relation name suggesting the
opposite); the saved view now joins that way. And saved view
JOINs that compared structural columns on `node_<kind>` tables
(e.g. `m.native_id`) were brittle: production discovery
routinely sets `MuxSessionNode.native_id` to a value distinct
from `MuxSessionId.native_id` (id is `tmux:<name>`, structural
is just `<name>`). All such JOINs in both `agent.rs` and the
saved views now reconstruct the Display form from the endpoint
JSON and compare against `node_<kind>.node_id`. Existing
`output::table` snapshot tests are the parity assertion; all
731 lib tests pass byte-for-byte, full integration suite green.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-004`
