---
id: CSP-076
title: Add a curated public re-export facade
status: Done
assignee: []
created_date: '2026-05-16 16:30'
labels:
  - p6
milestone: m-7
dependencies:
  - CSP-073
  - CSP-075
ordinal: 71000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a small `conspectus::api` module (or top-level
  `pub use` block in `src/lib.rs`) that re-exports the entry
  points named in `CSP-075`. Apply `#[doc(hidden)]` (or move to
  `pub(crate)`) on items the ADR marks internal. Keep the existing
  module paths working so current callers do not break.
- Tests: `cargo test --all-targets --all-features`; add a small
  doctest under `conspectus::api` that demonstrates a minimal
  library invocation (e.g. construct `LocalDiscoveryConfig::empty()`
  + call `discover_local_with` on a temp dir).
- Manual checks: `cargo doc --no-deps --open` and confirm the
  curated surface is the obvious entry point.
- Blockers: `CSP-073`, `CSP-075`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `conspectus::api` as the curated facade for
discovery, resolution, graph JSON, table rendering, config,
declared-link helpers, and graph model types. The facade includes a
doctest that performs a minimal temp-dir discovery with
`LocalDiscoveryConfig::empty()`. Test-only fixture/fake helpers now
stay reachable but are hidden from generated docs.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P6-004`
