---
id: CSP-041
title: Add representative agent and mux JSON snapshots
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-038
  - CSP-039
  - CSP-040
ordinal: 41000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: snapshot graph JSON for orphan sessions, mux-only sessions,
  one-to-many mux candidates, fork-linked sessions, and unresolved session
  lineage evidence.
- Tests: `cargo test --all-targets --all-features`; `cargo nextest run
  --all-targets --all-features`.
- Manual checks: review snapshots for stable ordering, readable provenance,
  preserved ambiguity, and no placeholder session nodes.
- Blockers: `CSP-038`, `CSP-039`, `CSP-040`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tests/harness_mux_snapshots.rs` with six end-to-end
snapshots driven by `discover_local_with` + `FakeTmux` + codex fixture
state covering orphan harness sessions, mux-only output, unavailable
tmux, exact-cwd session↔mux resolution, one-to-many mux candidates
(resolver picks the most recently active session per ADR 0006), and a
fork-associated session whose atelier harness entry stays as
unresolved parent/child lineage. All temp paths are normalized to
`/fixture` for stable ordering and no placeholder session nodes are
emitted.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-010`
