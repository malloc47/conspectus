---
id: CSP-137
title: 'ADR: graph snapshot persistence format and lifecycle'
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - p7
milestone: m-12
dependencies: []
ordinal: 377000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Accepted as ADR 0037 (snapshot-persistence-sqlite).
The ADR settles the canonical store at
`$XDG_DATA_HOME/conspectus/graph.sqlite`, schema versioning via
`PRAGMA user_version` aligned with the in-memory model, atomic
writes via SQLite transactions (no temp+rename), forward-only
migrations, the `--no-cache` / `--refresh` flag semantics, the
`provider_state` table + per-row `discovery_provider` /
`discovery_freshness_epoch` columns that feed CSP-138 / CSP-141,
and `VACUUM INTO` for rotation. The schema-apply scaffold
landed alongside CSP-271/CSP-272 (`src/query/schema.sql`,
`src/query/schema.rs`) and the canonical path resolver lives
at `src/query/runner.rs::graph_db_path`. Implementation of the
save/load lifecycle is `CSP-139`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P7-001`
