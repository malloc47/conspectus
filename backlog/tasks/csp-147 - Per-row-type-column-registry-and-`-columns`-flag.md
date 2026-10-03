---
id: CSP-147
title: Per-row-type column registry and `--columns` flag
status: Done
assignee: []
created_date: '2026-05-18 21:28'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 124000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/output/table.rs` grew a column registry keyed by
row-type. `ColumnSpec` records each column's stable `key`,
header label, one-line description, and `default` flag. The
`sessions`, `mux`, and `union` row-types each have their
registry slice plus a typed row-context struct
(`AgentRowCtx` / `MuxRowCtx` / `UnionRowCtx`) and a cell
extractor (`agent_cell` / `mux_cell` / `union_cell`). The three
`build_*_rows` functions now walk a `&[&'static str]` column
list and dispatch per cell. `RenderOptions::columns:
Option<Vec<&'static str>>` carries the selection; `None` falls
back to `default_columns(projection)`. `parse_columns_spec`
handles the `default` / `all` / `+name` / `-name` / explicit-list
token grammar from the backlog; `resolve_explicit_columns`
backs the config side. The `conspectus table <ROWS>` subcommands
gained `--columns LIST`. Config grew
`[table.<rows>].columns = [...]` (loaded via
`TableRowConfig::columns`); CLI flag overrides config when both
are present. Unknown columns error with the registered list
surfaced on stderr. Cached `attached_to_mux` on `SnapshotView` so
the mux "agents" cell stays O(1) per row. New unit tests cover
the parser (`default`/`all`/`+`/`-`/explicit-list/unknown/empty
tokens), `resolve_explicit_columns` validation, registry
defaults, and `render_with`/`with_columns` for both columnar
and card layouts. CLI integration tests cover `--columns`
override of defaults, delta tokens (with the
`--columns=-name,...` equals form so clap accepts the leading
dash), unknown-column errors, config-driven defaults, and CLI
overriding config. All 357 tests pass; existing snapshots stay
byte-for-byte stable because the registry's default sets match
the prior hard-coded headers and extractors. `docs/operations.md`
documents the flag and config knob.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-007`
