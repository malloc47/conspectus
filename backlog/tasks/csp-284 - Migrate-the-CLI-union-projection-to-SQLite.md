---
id: CSP-284
title: Migrate the CLI union projection to SQLite
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-282
  - CSP-283
ordinal: 477000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: same pattern for `Projection::Union`. Composes the
  agent/mux query paths over a `UNION ALL` shape.
- Tests: parity with existing union snapshots.
- Blockers: `CSP-282`, `CSP-283`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Production renderer at `src/output/union.rs` covers
all 7 cells. The agent/mux merge happens in SQL via the
pre-existing `v_nodes` view (a `UNION ALL` over every typed
node table) joined left to `node_agent_sessions`,
`node_mux_sessions`, and `aliases`; one query produces the
full ordered row stream and the cell extractor dispatches on
the row's `node_kind`. `ORDER BY` puts every agent row before
every mux row and breaks ties within a kind by Display-form
`node_id`. The `relationship` cell still uses a per-agent
preferred-`linked_to_mux` lookup that mirrors
`output::agent::fetch_mux_lookup`.
Substrate refactor: `pick_strongest` and
`confidence_precedence` are pulled into `output::render` so
agent / mux / union share one definition instead of three.
The in-memory `UnionRowSource`, `UnionRowCtx`, `union_cell`,
`build_union_rows`, `session_display_title`, and
`mux_session_label` are deleted. Existing `output::table`
snapshot tests are the parity check; all 731 lib tests pass
byte-for-byte and the full integration suite is green.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-006`
