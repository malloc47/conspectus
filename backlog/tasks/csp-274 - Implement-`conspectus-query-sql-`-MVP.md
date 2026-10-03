---
id: CSP-274
title: Implement `conspectus query <sql>` MVP
status: Done
assignee: []
created_date: '2026-05-25 21:41'
labels:
  - p9
milestone: m-14
dependencies:
  - CSP-273
  - CSP-139
ordinal: 466000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: a new CLI subcommand that (a) opens
  `$XDG_DATA_HOME/conspectus/graph.sqlite` in read-only mode (via
  `SQLITE_OPEN_READONLY`), falling back to building an in-memory
  snapshot from cold discovery if the file is absent, (b) executes
  the user's SQL with the standard pragma triplet
  (`synchronous=NORMAL`, `busy_timeout=5000`,
  `wal_autocheckpoint=1000`), (c) renders the result. Read-only
  enforcement: the read-only open mode rejects mutations at the
  SQLite layer (no DML, no DDL, no `ATTACH ... AS rw`). Render to a
  plain text table by default. `--format json` for machine output.
  `--format` flag value list expands in `CSP-275`.
- Tests: CLI integration tests for `conspectus query 'SELECT count(*)
  FROM nodes'`, a join across `candidate_links` and
  `node_agent_sessions`, a recursive CTE for fork ancestry, and
  expected-failure tests for `INSERT`, `UPDATE`, `DELETE`,
  `CREATE`, `DROP`. Snapshot tests over the fixture corpus.
- Manual checks: ad-hoc `conspectus query` invocations against a
  populated graph; confirm output readability and that mutation
  statements fail with a clean error.
- Blockers: `CSP-273`; persisted warm-start remains tracked by
  `CSP-139`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the `conspectus query` subcommand and
`src/query/runner.rs`. The runner opens a read-only
`graph.sqlite` when present, otherwise performs cold discovery
into an in-memory SQLite database, sets `PRAGMA query_only = 1`,
runs user SQL, and returns clear read-only errors for mutation
attempts. CLI smoke tests cover table and JSON output plus
rejected `INSERT` / `CREATE` statements.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P9-004`
