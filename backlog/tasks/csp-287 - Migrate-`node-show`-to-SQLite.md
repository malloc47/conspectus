---
id: CSP-287
title: Migrate `node show` to SQLite
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-280
ordinal: 480000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace the snapshot walks in `src/output/node_show.rs`
  with `Connection`-driven queries per node kind. Short-id
  resolution (`CSP-130`) keeps its current shape; the lookup
  moves to `SELECT … WHERE node_id LIKE ?`.
- Tests: parity with the existing `node show` snapshot corpus
  across every node kind.
- Blockers: `CSP-280`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Rewritten as
`resolve_node_id_from_conn(conn, input) -> Result<NodeId, NodeResolveError>`
+ `render_node_show_from_conn(conn, id, color) -> String`. The
existing `resolve_node_id` / `render_node_show` entry points
survive as thin bridges that materialize a snapshot to an
in-memory SQLite connection and delegate. `resolve_node_id`
walks `v_nodes` for hex-prefix and Display matches, then queries
`node_agent_sessions` / `node_mux_sessions` for label matches —
mirroring the in-memory `label_matches`. Per-kind summary
sections (`write_repo`, `write_checkout`, etc.) each issue a
single `SELECT … WHERE node_id = ?1` against the matching
typed table; the agent summary additionally joins to `aliases`
via the structural-field LEFT JOIN pattern from `output::agent`.
The candidate-links / resolved-relationships / diagnostics
sections issue filtered SELECTs (`source = ?` / `target_node =
?` / `conflict_source = ?`) with the bind being
`serde_json::to_string(&NodeId)` — the JSON-encoded endpoint
columns from ADR 0044 make text equality the right comparator.
A small `parse_display_via_typed_tables` helper reconstructs
typed `NodeId`s from a Display string by routing the kind
discriminator to the right `node_<kind>` PK lookup.
`reader::parse_node_id_json` is exposed `pub(crate)` so the
section renderers can turn JSON endpoint strings back into
`NodeId` Display form for the `→ <target>` / `← <source>`
cells. All nine node_show unit tests pass byte-for-byte; full
integration suite green.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-009`
