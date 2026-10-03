---
id: CSP-036
title: Add injectable tmux command execution
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-032
ordinal: 36000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: introduce a small command-runner seam for tmux discovery so tests can
  use fake output and production discovery can call `tmux` read-only.
- Tests: unit tests for unavailable tmux, command failures, invalid UTF-8 or
  malformed rows, and deterministic error diagnostics.
- Manual checks: verify no tests require a real tmux server.
- Blockers: `CSP-032`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `discovery::tmux` with a `TmuxRunner` trait, a `SystemTmux`
implementation that invokes `tmux list-sessions -F`, and a `FakeTmux`
test runner; outcomes are classified as `Sessions`, `Unavailable`
(binary missing or no server), or `Failed` with a stable diagnostic
string, and stdout is decoded lossily so invalid UTF-8 surfaces to the
parser rather than failing the runner.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-005`
