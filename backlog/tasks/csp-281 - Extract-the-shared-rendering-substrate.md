---
id: CSP-281
title: Extract the shared rendering substrate
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies: []
ordinal: 474000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: lift `RenderOptions`, `Layout`, the `ColumnSpec`
  registries, `render_rows`, `format_relative_age`,
  `node_short_id_from_display`, `unique_prefix_len`, `header_label`
  out of `src/output/table.rs` into a backend-agnostic module
  (`src/output/render/` or `src/output/substrate.rs`) consumed by
  both the in-memory and SQLite renderers during the migration.
  No behavior change; this is structural so the migration stories
  can land incrementally without circular dependencies. The
  `*_PUBLIC` aliases introduced by the spike are removed in favor
  of clean re-exports.
- Tests: existing renderer snapshot tests stay green byte-for-byte.
  A new module-boundary test confirms the substrate has no
  `crate::model::*` dependencies.
- Manual checks: `cargo build --no-default-features --features
  query` (verifies the substrate compiles without the in-memory
  renderer when the cfg is set up to allow it).
- Blockers: ADR 0043.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Substrate now lives at `src/output/render.rs`.
`output::table` re-exports the public surface so external callers
(`cli`, `node_show`, `tui`, `query::runner`) keep compiling
unchanged. `agent_sqlite.rs` imports from `super::render`
directly; the `SESSIONS_COLUMNS_PUBLIC` workaround is gone.
The `substrate_has_no_model_deps` test reads `render.rs` via
`include_str!` and flags any `use … crate::model` line that
sneaks in. All 732 lib tests pass byte-for-byte; full suite
green.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-003`
