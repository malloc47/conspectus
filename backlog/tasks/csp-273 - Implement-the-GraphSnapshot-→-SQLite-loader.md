---
id: CSP-273
title: Implement the GraphSnapshot → SQLite loader
status: Done
assignee: []
created_date: '2026-05-25 21:41'
labels:
  - p9
milestone: m-14
dependencies:
  - CSP-272
ordinal: 465000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: take a `&GraphSnapshot` and a `&mut Connection`,
  populate every table per `CSP-272`, in one transaction. The
  original plan expected to drive from `SnapshotIndex`; P10 later
  removed that selector layer, so the loader now walks the producer
  snapshot directly.
  Idempotency: re-running the loader replaces all rows
  (`DELETE FROM ...` followed by `INSERT`) inside the transaction.
  For partial eviction (`CSP-141`), the loader takes an optional
  `provider` filter and only touches rows owned by that provider.
  Use prepared statements + bind parameters; no string-built SQL.
  Performance target: a 1k-node / 5k-link graph loads in under
  50ms on a typical dev machine.
- Tests: round-trip tests — load each fixture snapshot, then
  `SELECT *` and compare against the Rust-side projection.
  Per-node-kind tests confirming every field survives a round trip.
  Idempotency tests (load, re-load, identical result).
  Provider-scoped reload tests confirming only the named provider's
  rows change.
- Manual checks: load a real snapshot and run a few `SELECT`s
  against it; confirm row counts match Rust-side counts.
- Blockers: `CSP-272`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `src/query/loader.rs`, `query::load`, and
`query::materialize_snapshot`. The loader writes graph nodes,
candidate links, resolved relationships, diagnostics, and aliases
with prepared statements and is covered by fixture round-trip tests
through the reader. `CSP-291` later removed the `SnapshotIndex`
dependency; the loader now iterates producer snapshots directly.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P9-003`
