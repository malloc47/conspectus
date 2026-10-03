---
id: CSP-152
title: '`conspectus columns <ROWS>` discovery subcommand'
status: Done
assignee: []
created_date: '2026-05-18 21:28'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 129000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
New `render_columns_listing(projection)` helper in
`src/output/table.rs` prints each registered column as
`<key>  <description>  (default)?` with the key column padded
for alignment. The CLI gained a top-level `conspectus columns
<ROWS>` subcommand that resolves the positional via
`config::Projection::parse` (so the same `sessions`/`mux`/
`union`/`prs`/`forks` tokens accepted by `conspectus table`
work here). Two new unit tests pin the `(default)` marker on a
default column and verify every registered key appears in the
listing for every projection. Two CLI integration tests cover
the listing across all five row-types (asserting both a
default-marked column and an opt-in column appear) and the
unknown-row-type error path. `docs/operations.md` documents
the new command. All 381 tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-012`
