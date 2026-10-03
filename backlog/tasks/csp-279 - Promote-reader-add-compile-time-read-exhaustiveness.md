---
id: CSP-279
title: Promote reader + add compile-time read exhaustiveness
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies: []
ordinal: 472000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: take the spike's `src/query/reader.rs` to production
  quality. Keep the round-trip equality test (`canonicalize()` on
  both sides) as the regression net. Add a per-table read helper
  (trait or macro) that pairs column-list constants with typed-row
  mappers so adding a column to `schema.sql` without updating the
  reader fails compilation — symmetric to the loader's exhaustive
  `let RepoNode { … } = repo;` destructuring. Move the spike's
  `parse_node_id` to the reader module unchanged; it survives only
  until `CSP-280` lands.
- Tests: round-trip equality over the existing loader fixture
  corpus (empty, full, link state variants, diagnostic variants,
  aliases). One synthetic test per node table that asserts adding
  a column without updating the reader fails to compile
  (`#[deny(unused)]` against the typed-row helper or equivalent).
- Manual checks: `conspectus dump --format json` against a real
  `graph.sqlite` reads back a snapshot that re-serializes equal
  to the JSON the loader produced from.
- Blockers: ADR 0043.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Spike reader promoted in place (module doc updated,
dead `_row` parameters dropped). Compile-time exhaustiveness
against the model is provided by the existing
`Ok(NodeKind { … })` constructions (a model field addition
breaks the build the same way the loader's destructure does).
Schema-side drift detection lives in `schema::TABLE_COLUMNS`
plus the new `schema_columns_match_constants` and
`table_columns_covers_every_relation_in_schema` tests — adding
or renaming a column / table / view in `schema.sql` without
updating the constants (or vice versa) fails the suite.
`parse_node_id` stays with a comment noting it disappears in
CSP-280.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-001`
