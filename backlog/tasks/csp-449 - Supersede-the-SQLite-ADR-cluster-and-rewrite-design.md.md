---
id: CSP-449
title: Supersede the SQLite ADR cluster and rewrite design.md
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 513000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADRs 0036, 0037, 0039, 0040, 0042, 0044 marked
**Superseded** by ADR 0082 with per-ADR notes explaining
exactly which commitments retired (the SQL query surface,
the on-disk SQLite persistence, the bundled libsqlite3
distribution carveouts, the vector-search surface, the
JSON-encoded NodeId schema). ADR 0043 marked **Partially
superseded** since the architectural claim "SQLite as
sole consumption surface" only retires on the user-facing
side (output renderers still consume
`query::materialize_snapshot` internally). ADR 0038
marked **Partially superseded** — Unix-socket framing /
commands / lifecycle policy stay in force; the WAL read
path and writer-fallback fork retire. Each supersession
note links back to ADR 0082 + ADR 0083 inline.
`docs/design.md` rewritten:
- §"Continuous Operation Mode" updated to describe the
  `snapshot` socket command, the daemon-or-cold-rebuild
  resolution chain, and the in-memory state model.
  Filesystem watchers move from "future optimization"
  to baseline (ADR 0081 shipped).
- §"Graph Snapshot Persistence" rewritten end-to-end:
  `graph.bin` + 32-byte header + atomic rename +
  bytecheck validation + in-memory `SnapshotState` /
  `SnapshotBytes` caches + daemon warm-restart. No more
  WAL, no more migrations, no more rotation.
- §"Query Surface" section deleted (~50 lines).
- §"Graph-to-View Slicing" rewritten to describe the
  daemon-snapshot consumption path. Later CSP-448.02/CSP-448.03/CSP-448.04
  work removed the internal `materialize_snapshot` path
  entirely.
- §"Remaining Design Questions → Continuous Operation
  And Snapshot Persistence" pruned to the three open
  questions that actually remain (eviction granularity,
  resolver re-run cadence, adaptive intervals);
  everything ADR 0082/0083 settled retires.
- §"Discovery Strategy" / §"State And Persistence"
  mention of "JSON, SQLite, node detail" dropped to
  "JSON, node detail" — SQLite is no longer a
  first-class export channel.
Tests: docs-only; `git diff --check` clean. Full suite
still green (modulo the pre-existing
pin_state_matrix_agent_table_snapshot failure from
commit 84198ea).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-012`
