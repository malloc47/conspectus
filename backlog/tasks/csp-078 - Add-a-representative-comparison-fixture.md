---
id: CSP-078
title: Add a representative comparison fixture
status: Done
assignee: []
created_date: '2026-05-16 16:30'
labels:
  - p6
milestone: m-7
dependencies:
  - CSP-071
ordinal: 73000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add an integration test that runs `discover_local_with`
  on a temp-dir fixture mimicking an Atelier workspace (atelier
  config + fork index + a fake harness session + a `FakeTmux`)
  and snapshots the rendered graph JSON plus all three session
  table projections. Path-normalize to `/fixture` for byte-stable
  reruns. The intent is to give Atelier delegation a concrete
  target to validate against during its own work.
- Tests: `cargo nextest run --all-targets --all-features`.
- Manual checks: review the new snapshots for stable ordering and
  preserved evidence/ambiguity.
- Blockers: `CSP-071`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tests/atelier_delegation_snapshots.rs`, which
builds an Atelier-style workspace with two git repos, a checkout
fork, unresolved codex lineage metadata, a fake codex session, and a
matching `FakeTmux` row. The test snapshots rendered graph JSON plus
agent, mux, and union session table projections with temp paths
normalized to `/fixture`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P6-006`
