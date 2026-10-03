---
id: CSP-570
title: Rename `GraphDb`
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 579000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: `GraphDb`, `graph_db()`, and the `database` field are
  names left over from the SQLite era for what is now an
  `Rc<GraphSnapshot>` handle.
- Plan: rename to `SnapshotHandle` / `snapshot_handle()`. Mechanical.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`SnapshotHandle` / `snapshot_handle()`; TUI locals named
`database` became `handle`, and the status messages now say "no graph
loaded yet".
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-017`
