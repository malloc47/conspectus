---
id: CSP-401
title: Audit GraphSnapshot round-trip and force coverage on future fields
status: Done
assignee: []
created_date: '2026-06-07 02:38'
labels:
  - p10-fu
milestone: m-15
dependencies:
  - CSP-279
ordinal: 487000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Context: two silently-latent round-trip gaps shipped before any
  test caught them. `Diagnostic::PinUnbound` (and the three sibling
  pin diagnostic kinds) was emitted by the resolver and written by
  the loader but unhandled by the reader, so `read_snapshot` failed
  the moment a user created their first pin. `GraphSnapshot.pins`
  was never inserted into SQLite at all, so the TUI's
  `build_sessions_tree_from_conn` (which reads via `read_snapshot`)
  saw zero pins and the synthetic "Pins" group never rendered.
  Both bugs slipped past `CSP-279`'s `schema_columns_match_constants`
  and `table_columns_covers_every_relation_in_schema` drift catches
  — those check schema-vs-constants, not model-vs-tables.
- Scope: walk every top-level `GraphSnapshot` field (`nodes`,
  `candidate_links`, `resolved_relationships`, `diagnostics`,
  `aliases`, `pins`) and assert each one round-trips losslessly
  through `materialize_snapshot` → `read_snapshot`. Walk every
  enum variant the snapshot can carry — `GraphNode`, `LinkState`,
  `LinkEndpoint`, `Diagnostic`, `PinBinding`, `Provenance`,
  `Confidence`, `Freshness`, `RelationKind`, `RuntimeProcessRole`,
  `SessionKind` — and assert each variant round-trips. Add an
  explicit destructure of `GraphSnapshot` in the matrix test so
  adding a new top-level field is a compile error until the test
  handles it (mirror the loader's exhaustive
  `let RepoNode { … } = repo;` pattern at the snapshot level).
  Add an explicit exhaustive `match` on each enum-variant matrix
  so adding a variant is a compile error until the matrix covers
  it.
- Tests:
  - `graph_snapshot_round_trips_fully_populated`: one test that
    builds a snapshot populated with at least one of every node
    kind, link state, diagnostic variant, pin binding state, and
    alias, materializes it, reads it back, and asserts equality
    via `canonicalize()`.
  - `every_diagnostic_variant_round_trips`: exhaustive match on
    `Diagnostic`, one row in the test matrix per variant, round-
    tripped individually so a mismatch isolates the broken kind.
    Covers the regression that yesterday's `details: TEXT` patch
    addressed.
  - `every_pin_binding_state_round_trips`: same pattern for the
    three `PinBinding` variants plus `binding = None`.
  - `every_link_state_round_trips` and
    `every_link_endpoint_round_trips`: if not already covered,
    mirror the structure.
  - `clear_all_handles_every_materialized_table`: builds a fully-
    populated database, runs `load(empty_snapshot, &mut conn)`,
    asserts every materialized table is empty. Catches the
    regression where `pins` was missing from `clear_all`'s table
    list yesterday.
  - Maintenance lint: a synthetic test that uses
    `let GraphSnapshot { nodes: _, candidate_links: _,
    resolved_relationships: _, diagnostics: _, aliases: _,
    pins: _ } = …;` to bind every field — adding a snapshot field
    without updating the test fails to compile.
- Out of scope: schema columns for individual model fields beyond
  what the round-trip needs. Pin-quality SQL queryability
  columns (the columnar fields I added to `pins`) are kept where
  they already exist; this story doesn't add or remove them.
- Blockers: none. Independent hardening pass on the layer
  `CSP-279` audited but didn't fully nail down.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`full_snapshot_round_trips` extended to cover all 4
pin-* diagnostic variants (including PinUnbound with and
without last_session) and all 3 PinBinding states plus the
pre-resolve None case. Seven new audit tests in
`src/query/reader.rs`:
`graph_snapshot_field_drift_guard` (destructures GraphSnapshot
so a new top-level field fails to compile),
`every_diagnostic_variant_round_trips`,
`every_pin_binding_state_round_trips`,
`every_link_state_round_trips`,
`every_link_endpoint_round_trips`,
`every_session_kind_round_trips`,
`every_runtime_process_role_round_trips`,
`clear_all_handles_every_materialized_table`. Each uses an
exhaustive match on its enum so adding a variant is a
compile error until the matrix covers it. SessionKind and
RuntimeProcessRole tests in particular catch the reader's
silent-`None` asymmetry on unknown strings (writer is
exhaustive; reader fan-in maps unknowns to None). The
`clear_all` test catches the recent `pins` missing-table
regression by reloading-with-empty and asserting every
materialized table is empty afterwards.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-FU-002`
