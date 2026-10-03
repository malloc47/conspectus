---
id: CSP-143
title: Implement CLI ↔ server snapshot read path
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - p7
milestone: m-12
dependencies:
  - CSP-142
ordinal: 383000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: when a server is running (detected by an existing
  transport endpoint), `conspectus session` / `conspectus graph`
  / `conspectus node show` read the server's current snapshot
  rather than performing in-process discovery. Without a server,
  the CLI behaves as today (with the warm-start from `CSP-139`).
  The transition must be transparent to users; a stale-server or
  schema-mismatch condition falls back to one-shot mode with a
  one-line stderr hint.
- Tests: CLI integration tests covering server-present and
  server-absent paths, schema-version mismatch fallback, and
  transport-error fallback. End-to-end tests confirming a CLI
  invocation against a running server returns the same JSON as
  the equivalent one-shot run for the same graph state.
- Manual checks: confirm `conspectus session` latency drops
  when a server is running.
- Blockers: `CSP-142`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): landed through the Phase 11 socket
`snapshot` command; one-shot commands take the daemon snapshot via
`try_daemon_snapshot` (`src/cli/mod.rs`). The text above predates the
`table` rename of `session`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P7-007`
