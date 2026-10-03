---
id: CSP-280
title: JSON-encoded NodeId foreign references (ADR 0044)
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-279
ordinal: 473000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace the `*_node_id TEXT` foreign-reference columns in
  `candidate_links`, `resolved_relationships`, `diagnostics`, and
  `aliases` with a JSON column holding the serde-serialized typed
  value plus a `GENERATED ALWAYS AS (json_extract(col, '$.type'))
  STORED` discriminator column. The loader writes via
  `serde_json::to_string(&link.source)` etc.; the reader recovers
  typed values via `serde_json::from_str::<NodeId>` and
  `reader::parse_node_id` is deleted. `resolved_relationships` PK
  becomes `(source, relation, target)`; `aliases` PK becomes
  `node`. Rewrite `v_mux_attachments`, `v_pr_by_branch`,
  `v_fork_ancestry`, and `v_workspace_member_repos` to use
  `json_extract` for structural joins; `v_sessions_with_repo` and
  `v_nodes` are unaffected. Add expression indexes for each
  rewritten view's access pattern. Bump `SCHEMA_VERSION` from 2
  to 3. Update `TABLE_COLUMNS` for the new shape; the
  `schema_columns_match_constants` test from CSP-279 catches the
  schema-side drift.
- Tests: round-trip equality test from CSP-279 stays green
  byte-for-byte. Add `every_node_id_variant_round_trips_through_json`
  covering all 8 `NodeId` variants (catches serde shape drift
  that the schema-side test cannot see). Add a test covering a
  `RepoId` whose `common_dir` contains `@`, `:`, `#`, and `/` —
  values `parse_node_id` would mishandle — and confirm the JSON
  encoding preserves them losslessly through load → read. Saved
  view smoke tests stay green; add a fixture-driven test that
  asserts `v_mux_attachments` returns the same rows after the
  JOIN rewrite.
- Manual checks: run a fresh discovery cycle on a real corpus
  and confirm endpoint columns inspect cleanly via `sqlite3`;
  confirm `--similar-to` and the existing saved-view queries
  behave identically.
- Blockers: `CSP-279`. ADR: 0044.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`candidate_links`, `resolved_relationships`,
`diagnostics`, and `aliases` now store endpoints as JSON via
`serde_json::to_string(&node_id)`. STORED GENERATED `*_kind`
columns expose `json_extract(col, '$.type')` for indexed
filtering; `schema_columns_match_constants` runs against
`PRAGMA table_xinfo` so generated columns participate in
drift detection. `parse_node_id` is gone; the reader uses
`serde_json::from_str::<NodeId>` everywhere. `SCHEMA_VERSION`
bumped 2 → 3. Saved views `v_mux_attachments`,
`v_pr_by_branch`, `v_fork_ancestry`, and
`v_workspace_member_repos` rewritten to use structural joins
via `json_extract` against the typed node tables; the
`idx_candidate_links_target_mux_native_id` expression index
supports `v_mux_attachments`'s join path. The
`every_node_id_variant_round_trips_through_json` test in
`query::reader` covers every NodeId variant; the
`endpoints_with_separator_chars_round_trip_through_json` test
confirms `RepoId` / `BranchId` / `ForgePrId` values containing
`:` `@` `#` `/` round-trip losslessly through the load → read
cycle (which the old `Display`-parser approach could not
have done). `docs/query-guide.md` updated for the new column
set and the recursive-CTE example. 733 lib tests pass; all
integration test binaries green.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-002`
