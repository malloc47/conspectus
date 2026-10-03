---
id: CSP-063
title: Preserve read-only command invariants
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-062
ordinal: 59000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: explicitly verify `conspectus graph` and `conspectus session`
  never create or mutate `.conspectus.toml`, user config files, or
  cache directories while loading declared evidence.
- Tests: CLI integration tests for graph/session from a clean repo,
  a repo with existing config, an orphan/non-repo cwd, and explicit
  `--scan-root` values; assert filesystem mtimes/content stay
  unchanged.
- Manual checks: `cargo run -- graph --format json`; `test ! -e
  .conspectus.toml`; repeat with `cargo run -- session`.
- Blockers: `CSP-062`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added CLI smoke coverage proving `graph` and `session`
do not create `.conspectus.toml` or user config in a clean repo,
and do not mutate an existing project config with declared-link
state when run from either cwd or explicit `--scan-root`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-004`
