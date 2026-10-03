---
id: CSP-271
title: 'SQLite integration spike (bundled build, WAL, lifecycle)'
status: Done
assignee: []
created_date: '2026-05-25 21:41'
labels:
  - p9
milestone: m-14
dependencies: []
ordinal: 463000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: integrate the `rusqlite` crate behind a `query` Cargo feature
  (per ADR-D) with the `bundled` sub-feature on. Confirm SQLite
  3.51.3+ ships and add a CI assertion that fails the build below
  that floor (the 3.51.3 fix addresses the WAL-reset corruption bug
  that affected 3.7.0–3.51.2 in multi-writer / multi-checkpointer
  scenarios — exactly our server + CLI pattern). Measure
  release-binary size with and without the feature. Validate that
  `nix develop` produces a working build. Confirm WAL mode behavior
  by running a two-process smoke test (process A holds a writer
  connection while process B opens a reader; assert no blocking).
  Record findings as a short note appended to the distribution ADR.
  No graph schema yet; the spike is operational.
- Tests: integration tests that open an in-memory connection, run
  `SELECT 1`, and that open a file-backed WAL-mode connection from
  two processes. Build matrix check that the non-feature build is
  unchanged. CI assertion on bundled SQLite version.
- Manual checks: `cargo build` with and without `--features query`;
  inspect release binary size; run the two-process smoke test
  manually.
- Blockers: ADR-A (engine selection), ADR-D (library API), ADR-E
  (distribution amendment).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Accepted ADRs 0036, 0039, 0040, and 0041; added the
default-on `query` feature and the SQLite query module. The spike
coverage now includes a bundled SQLite version floor
(`MIN_SQLITE_VERSION = 3.51.3`), an in-memory `SELECT 1` smoke
test, and a WAL reader/writer concurrency smoke test.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P9-001`
