---
id: CSP-515
title: >-
  Per-file `(mtime, size)` cache for claude + codex session header/tail scans
  (`9b9eec3`)
status: Done
assignee: []
created_date: '2026-07-29 02:07'
labels:
  - h-serve-perf
milestone: m-18
dependencies: []
ordinal: 539000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
On this operator's box 93 of 96 session files were dormant; caching drops cold-file opens to zero on dormant paths.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SERVE-PERF-007`
