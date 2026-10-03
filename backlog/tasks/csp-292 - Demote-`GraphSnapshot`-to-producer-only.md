---
id: CSP-292
title: Demote `GraphSnapshot` to producer-only
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-291
ordinal: 485000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: gate the public re-export so library callers who only
  want to render get a `Connection`-flavored API, not a snapshot.
  `GraphSnapshot` remains the resolver's input/output type and
  `conspectus dump --format json`'s wire shape (built via
  `read_snapshot()`), but no consumer holds it across a CLI
  invocation. Update `src/api.rs` accordingly. Refresh
  `docs/library-api.md` and `docs/design.md` to reflect the
  consumer-side contract.
- Tests: library-API surface tests confirm `GraphSnapshot` is no
  longer reachable from rendering entry points.
- Manual checks: review the updated library-API doc; confirm an
  external Rust caller building a TUI substitute can succeed
  against the new surface.
- Blockers: `CSP-291`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
TUI app state now stores a SQLite `GraphDb` wrapper and
recomputes detail from `build_node_detail_from_conn`; refresh
materializes the resolved producer snapshot into SQLite before
dispatching `SetData`. CLI table and `node show` paths materialize
SQLite and call the connection-backed render/inspection APIs.
`GraphSnapshot` is no longer re-exported from `api`; it remains
available from `model` for discovery/resolver/dump/test fixtures.
`output::table` exposes `render_conn` / `render_with_conn` as the
canonical renderer surface while preserving snapshot bridges for
fixture-heavy producer-side tests.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-014`
