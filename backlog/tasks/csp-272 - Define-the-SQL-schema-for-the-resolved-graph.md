---
id: CSP-272
title: Define the SQL schema for the resolved graph
status: Done
assignee: []
created_date: '2026-05-25 21:41'
labels:
  - p9
milestone: m-14
dependencies:
  - CSP-271
ordinal: 464000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: produce a DDL for tables `nodes`, `node_repos`,
  `node_checkouts`, `node_workspaces`, `node_agent_sessions`,
  `node_mux_sessions`, `node_branches`, `node_forks`,
  `node_forge_prs`, `candidate_links`, `resolved_relationships`,
  `diagnostics`, `aliases`, `provider_state`. One table per node
  kind for queryability; a `v_nodes` view that unions them by
  `node_id` and `node_kind`. Columns mirror the Rust model
  field-for-field where possible; polymorphic blobs
  (`SourceMetadata.fields`, `UnresolvedEndpoint.metadata`) land in
  `TEXT` columns holding JSON, queryable via `JSON_EXTRACT`. Stable
  indexes on `(source_node_id, relation)`,
  `(target_node_id, relation)`, `(provider, freshness_epoch)`,
  `(last_active_epoch DESC)`. Schema version recorded via
  `PRAGMA user_version` aligned with `GraphSnapshot`'s schema
  version. DDL lives under `src/query/schema.sql` (or a Rust
  constant) so the loader can apply it deterministically.
- Tests: a DDL apply test that runs the schema against a fresh
  in-memory connection and confirms no errors. A test asserting
  every `NodeKind` and `RelationKind` variant maps to a column /
  enum entry in the DDL so adding a new variant fails compilation.
- Manual checks: `sqlite3 :memory: < schema.sql` lists the expected
  tables / indexes.
- Blockers: `CSP-271`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `src/query/schema.sql` and `src/query/schema.rs`
with typed node tables, candidate/resolved/diagnostic/alias
tables, curated saved views, schema versioning, generated
endpoint-kind columns, JSON-encoded endpoint references, and
schema-drift tests that compare the DDL against Rust constants.
ADR 0044 records the JSON endpoint decision.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P9-002`
