---
id: CSP-462
title: Dedupe the copy-pasted micro-helpers
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies: []
ordinal: 93000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-04 (commit `c82c9f6`). All three helper
  families collapsed: `snapshot_fragment` (7 copies →
  `impl From<GraphSnapshot> for GraphFragment`),
  `current_epoch` (5 copies → single canonical
  `crate::discovery::current_epoch` re-exported from every
  prior location for source-compat), `path_string` (5 copies
  → `crate::discovery::path_to_string`). Net −202 / +167
  across 12 files.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HYG-001`
