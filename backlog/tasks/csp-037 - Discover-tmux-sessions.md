---
id: CSP-037
title: Discover tmux sessions
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-036
ordinal: 37000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: parse `tmux list-sessions` format output into `MuxSession` nodes,
  including session name, activity metadata when available, and root/cwd path
  evidence.
- Tests: fake-command tests for zero sessions, one session, multiple
  sessions, paths with spaces, missing root/cwd fields, and unavailable tmux.
- Manual checks: create a temporary tmux session and inspect mux-session JSON.
- Blockers: `CSP-036`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a tab-separated `TMUX_LIST_FORMAT`
(`#{session_name}\t#{session_path}\t#{session_activity}\t#{session_created}`),
a `parse_list_sessions` parser that yields rich `TmuxSessionRow` values
(preserving activity/creation epochs and paths with spaces), and a
`TmuxDiscovery` provider that emits one `MuxSession` node per row while
surfacing `Available`/`Unavailable`/`Failed` status to callers that need
diagnostics. All tests use `FakeTmux` so no real tmux server is required.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-006`
