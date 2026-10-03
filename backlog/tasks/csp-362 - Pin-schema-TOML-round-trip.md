---
id: CSP-362
title: Pin schema + TOML round-trip
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-361
ordinal: 295000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `src/pins.rs` with `PinEntry`, `PinMux`, `PinLaunch`
  serde models matching ADR 0057. `schema_version`, unknown-field
  tolerance, malformed-entry diagnostics, validation
  (non-empty `id` / `display_name` / `harness`, absolute `cwd`,
  `mux.backend == "tmux"` in v1, `mux.name` non-empty,
  `mux.socket_name` non-empty when present, duplicate-id rejection,
  duplicate-(`mux.backend`, `mux.name`, `mux.socket_name`)
  rejection). Round-trip TOML decode/encode preserves unrelated
  sections.
- Tests: unit tests for happy-path TOML, missing-section default,
  unknown keys, malformed entries, duplicate id, duplicate mux
  triple, empty / missing required fields, the default-socket vs
  non-default-socket cases, and a round-trip preserving `[session]`
  / `[declared]` / `[aliases]` siblings.
- Blockers: `CSP-361`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/pins.rs` defines the v1 pin schema, validation,
parse, round-trip, upsert, and remove helpers with unit coverage.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-002`
