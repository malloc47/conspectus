---
id: CSP-250
title: Define `RowFilter` predicate + dimension types in a crate-public module
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies: []
ordinal: 446000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: introduce `src/filter.rs` with `RowFilter`,
  `HarnessFilter::Any(Vec<String>)`,
  `MuxStateFilter::Any(Vec<MuxStateKey>)`, and
  `MuxStateKey { Attached, Ambiguous, Unmuxed }` per ADR 0031.
  Predicate evaluation runs against an `AgentSessionNode` plus
  its resolved mux state. Apply the predicate in
  `build_sessions_tree` before bucket emission so empty groups
  collapse naturally. No new dependencies (ADR 0024 policy).
- Tests: unit tests over existing sessions fixtures for
  claude-only narrowing, max-age cutoff at the boundary minute,
  unmuxed-only, ambiguous-only, the empty-result case, and the
  intersection of all three v1 dimensions.
- Blockers: ADR 0031.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in `src/filter.rs` with `RowFilter`,
`HarnessFilter::Any`, `MuxStateFilter::Any`, and
`MuxStateKey { Attached, Ambiguous, Unmuxed }` per ADR 0031.
Predicate applies in `build_sessions_tree` so empty groups
collapse. Unit tests cover the v1 dimensions and their
intersection.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-001`
