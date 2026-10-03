---
id: CSP-291
title: 'Retire `SnapshotIndex`, `SnapshotView`, `SessionsIndex`'
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies:
  - CSP-283
  - CSP-289
  - CSP-290
ordinal: 484000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: delete the in-memory selector layer once no consumer
  depends on it. Producer-side discovery and the resolver may
  keep an internal selector if useful (the loader doesn't need
  one). Remove the dual-renderer plumbing; delete
  `src/output/agent_sqlite.rs` (its production replacement is
  `src/output/table.rs`'s new implementation).
- Tests: `cargo build` succeeds; the existing test suite stays
  green; `cargo +nightly udeps`-style dead-code sweep finds
  nothing residual.
- Blockers: `CSP-283`..`CSP-289` complete; `CSP-290` is deferred
  until the non-session TUI builders exist.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Deleted `src/model/index.rs` and removed the
`SnapshotIndex` re-export. The SQLite loader now iterates
`GraphSnapshot.nodes` directly; the sessions TUI builder keeps a
private per-call data helper for its typed assembly instead of the
shared selector layer. `SnapshotView` / `SessionsIndex` already had
no production symbols left.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-013`
