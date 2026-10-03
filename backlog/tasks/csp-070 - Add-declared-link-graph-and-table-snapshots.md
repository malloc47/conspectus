---
id: CSP-070
title: Add declared-link graph and table snapshots
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-062
  - CSP-069
ordinal: 66000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add representative snapshots for local declared links,
  global declared links, local-over-global precedence, ignored
  discovered candidates, overridden candidates, unresolved declared
  endpoints, and session-table rendering with declared mux/PR
  relationships.
- Tests: `cargo test --all-targets --all-features`; `cargo nextest
  run --all-targets --all-features`.
- Manual checks: review snapshots for stable ordering, readable TOML
  provenance, and preserved discovered evidence.
- Blockers: `CSP-062`, `CSP-069`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tests/declared_snapshots.rs` with seven scenarios
driven by `discover_local_with` plus an injected `ConfigLoader`
and `FakeTmux`: local-declared with matched target, global
declared, local-over-global precedence (local wins resolution and
global stays as a competing link plus `Conflict` diagnostic),
ignored declared link, overridden declared link, unresolved
declared endpoint when no discovery providers run, and an
agent-projection table rendering the declared mux relationship.
Temp paths normalize to `/fixture` so reruns stay byte-stable.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-011`
