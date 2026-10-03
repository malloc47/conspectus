---
id: CSP-518
title: mtime-cache the opencode.db SQL scan (`148763d`)
status: Done
assignee: []
created_date: '2026-07-29 02:07'
labels:
  - h-serve-perf
milestone: m-18
dependencies: []
ordinal: 542000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`read_sqlite_sessions` full-table-scanned the 25 MB DB on every harness cycle; WAL mode keeps the main-file mtime stable so the cache hits ~100%. **37 MB/s → 549 KB/s** — the second biggest single win.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SERVE-PERF-010`
