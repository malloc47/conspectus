---
id: CSP-446
title: Daemon warm-start from the on-disk artifact
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 505000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: covered by the CSP-448.01 daemon/cache validation
  suite and subsequent server coverage. The socket path
  continues to return `snapshot_unavailable` only when no
  warm artifact and no published in-memory snapshot exist.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Delivered as part of CSP-448.01. On startup, the
daemon attempts `snapshot::open_mmap` against `graph.bin`
and seeds `SnapshotState` before the first per-class cycle.
Missing files, malformed artifacts, and version mismatches
log and fall through to first-cycle cold-rebuild semantics.
Once a cycle succeeds, `publish_snapshot` refreshes both
the in-memory graph state and socket-served snapshot bytes.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-009`
