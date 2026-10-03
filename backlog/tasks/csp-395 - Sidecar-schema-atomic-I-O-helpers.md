---
id: CSP-395
title: Sidecar schema + atomic I/O helpers
status: Done
assignee: []
created_date: '2026-06-05 22:52'
labels:
  - h-pin-resume
milestone: m-11
dependencies: []
ordinal: 336000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `src/pin_bindings.rs` (or a sibling module under
  `src/pins/`) with the per-pin JSON record per ADR 0058 §Sidecar
  shape: `schema_version: u32`, `pin_id`, `mux_name`,
  `mux_socket`, `session_id`, `harness`, `observed_epoch`. Serde
  models with unknown-field tolerance on read, validation on
  write, malformed-file diagnostic that leaves the sidecar alone.
  Atomic write helpers (tempfile + rename) for per-pin files
  under `$XDG_CACHE_HOME/conspectus/pin-bindings/<pin_id>.json`.
  Skip-on-unchanged comparison to avoid churning quiet cycles.
  Pure read/write — no discovery, no launch wiring.
- Tests: unit tests for happy-path round-trip, unknown-field
  tolerance, malformed-file refusal, atomic write under
  interruption simulation, skip-on-unchanged, path resolution
  against an `$XDG_CACHE_HOME` override fixture.
- Blockers: ADR 0058 (Accepted).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/pin_bindings.rs` exposes
`PinBindingRecord`, `PinBindingsCache` (mirrors
`ConfigLoader`'s env-override shape), and
`parse_record / to_json / read / write / delete` helpers built
on the shared `declared::write_atomic` primitive. 21 unit tests.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-RESUME-001`
