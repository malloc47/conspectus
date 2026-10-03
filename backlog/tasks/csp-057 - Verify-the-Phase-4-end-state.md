---
id: CSP-057
title: Verify the Phase 4 end state
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-054
  - CSP-055
  - CSP-056
ordinal: 55000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: run the full Phase 4 automated and manual check set and
  record follow-up tasks instead of expanding Phase 4 scope.
- Tests: `just check`.
- Manual checks: run the four `cargo run -- session` smoke commands
  from `docs/implementation/phase-04-forge-and-table-views.md`, plus
  `cargo run -- graph --format json` from a repo with a real open
  PR. Confirm the PR node and `BranchHasForgePr` link appear in JSON
  and surface in the session-table projection.
- Blockers: `CSP-054`, `CSP-055`, `CSP-056`.
- Follow-up: live `gh` was unauthenticated in the dev shell, so
  the smoke run did not exercise real PR retrieval. The forge
  code path is exercised by 14 unit/library tests and 3 JSON
  snapshots using `FakeGh`; verifying against a real
  authenticated `gh` belongs in a follow-up smoke test run by a
  user with credentials, not in Phase 4 scope.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`nix develop --command just check` passed with 210 tests.
`cargo run -- session`, `--projection agent`, `--projection mux`,
and `--projection union` all rendered tables against live local
state (claude-code agent sessions, no mux/PR rows because the
local `gh` is unauthenticated and the smoke test had no tmux
server). `cargo run -- graph --format json` emitted repo /
checkout / branch / agent_session / mux_session nodes plus 14
resolved relationships. Discovery remained read-only.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-013`
