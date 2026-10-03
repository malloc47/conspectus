---
id: CSP-380
title: Snapshot and JSON coverage
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-369
  - CSP-373
  - CSP-374
  - CSP-375
ordinal: 316000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend `tests/declared_snapshots.rs` (or sibling file
  `tests/pins_snapshots.rs`) with scenarios covering bound,
  unbound, stale-mux, ambiguous, drift, duplicate, local-over-
  global, declared-override-via-bind, and a non-default-socket
  pin. Graph JSON snapshots and table-projection snapshots both
  covered.
- Tests: `cargo nextest run --all-targets --all-features`.
- Blockers: `CSP-369`, `CSP-373`, `CSP-374`, `CSP-375`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`tests/pins_snapshots.rs` snapshots a combined pin
state matrix covering bound, unbound, stale-mux, ambiguous,
drift, and non-default-socket pins in graph JSON, plus the
sessions table projection for bound pin relationships. Dedicated
discovery snapshots cover duplicate pin config diagnostics and
local-over-global store shadowing, and a resolver snapshot covers
declared-override-via-bind choosing the `LocalDeclared`
`linked_to_mux` candidate over strong discovery.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-020`
