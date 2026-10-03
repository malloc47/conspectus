---
id: CSP-140
title: 'ADR: continuous server mode architecture and transport'
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - p7
milestone: m-12
dependencies: []
ordinal: 380000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Accepted as ADR 0038 (cli-server-transport-wal). The
ADR settles the WAL-backed read path (every process opens
`graph.sqlite` read-only, no IPC for queries), the writer
contract (server owns the writer connection when running,
one-shot CLI takes the lock when absent), the length-prefixed
JSON mutation socket at
`$XDG_RUNTIME_DIR/conspectus/server.sock`, the pragma triplet
(`synchronous=NORMAL`, `busy_timeout=5000`,
`wal_autocheckpoint=1000`) — already applied in
`src/query/runner.rs::apply_query_pragmas` — the `[server]` /
`[server.intervals]` config shape, per-provider failure
isolation expectations, and the no-auto-spawn lifecycle. The
"absence of a server is not an error" guarantee falls out of
the WAL read path. Implementation of `conspectus serve` is
`CSP-142`; the CLI client integration is `CSP-143`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P7-004`
