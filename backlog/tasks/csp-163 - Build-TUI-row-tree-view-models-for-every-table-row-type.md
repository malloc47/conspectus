---
id: CSP-163
title: Build TUI row tree view-models for every table row-type
status: Done
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies:
  - CSP-162
ordinal: 390000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add pure row-tree builders for `sessions`, `mux`, `union`,
  `prs`, and `forks`. The builders consume a resolved `GraphSnapshot`
  and produce stable row ids, labels, depth, row kind, primary node id,
  sort keys, and compact status fields. Sessions view groups by the
  v1 "project" answer from CSP-160 and nests known lineage/fork history.
  Mux view groups by mux session and nests attached agent sessions.
  PR/fork/union views preserve parity with the existing table row-types
  without scraping rendered table text.
- Tests: unit tests with existing fixtures covering empty graph,
  orphan session, mux-only, attached session, fork lineage, PR-linked
  branch, and ambiguous links. Snapshot the pure row-tree structures
  rather than terminal output.
- Blockers: `CSP-162`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): row builders for all five row types
live in `src/tui/rows/` (first slice `50f206c`).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-004`
