---
id: CSP-221
title: >-
  Add opencode server/ACP attribution adapter if the audit proves a stable
  non-mutating query
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-219
ordinal: 258000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**Closed as won't-do 2026-05-31.**

- Scope: if `CSP-219` confirms opencode `serve`, `attach`,
  or ACP can report active session identity for a running TUI or
  headless server, implement an optional adapter that maps the
  server session id back to a `MuxSession`. Prefer documented
  attach URLs, pid/socket evidence, or server metadata over
  guessing from ports. Keep plain opencode TUI support on the
  existing fd / command / cwd path.
- Tests: fake server tests for active session, multiple projects,
  unavailable server, auth/connection failure, and ambiguous
  session responses.
- Manual checks: launch opencode in the supported server mode and
  confirm the query does not create transcript records or alter
  session recency.
- Blockers: `CSP-219`.
- **closure rationale**: supplanted by the plugin sidecar path in
  `CSP-229` (live-verified 2026-05-31). The opencode plugin
  runs in-process inside every TUI/server/ACP launch mode, writes
  `session.created`/`updated`/`idle`/`status`/`compacted`
  observations to the hook sidecar without HTTP/socket discovery
  or auth, and the existing `discovery::hook_sidecar` reader
  already attributes the records by tmux pane. An HTTP/ACP
  adapter would duplicate this evidence at higher cost (port
  discovery, auth-token plumbing, multi-server polling). Reopen
  only if the plugin distribution becomes untenable (e.g.
  opencode removes the plugin surface).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-MUXPROC-007`
