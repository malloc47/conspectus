---
id: CSP-663
title: Report mux probe status per backend and socket
status: Done
assignee:
  - '@claude'
created_date: '2026-10-22 01:10'
updated_date: '2026-10-22 12:45'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-656
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: high
type: feature
ordinal: 721000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
History marks a mux gone only when a successful probe of its backend and socket omitted it. Today discovery can't tell "tmux has no server" (every mux really is gone, as after a reboot) from "tmux failed" or "this socket was never probed". Rename continuity also needs tmux's own session id.

- Scope: add `#{session_id}` to `TMUX_LIST_FORMAT`; report probe status per backend and socket as success, empty (no server), or failure; extend the probed socket set with sockets named in history records (part of CSP-382); report zellij probe status the same way.
- Tests: parser tests for the new field; probe-status tests for no-server, failure, and success; a test that a history socket is probed.
- Manual checks: kill the tmux server and confirm the probe reports empty success, not failure.
- Blockers: the history ADR.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Discovered tmux muxes carry tmux's `session_id`
- [x] #2 Each probed backend and socket reports success, empty, or failure, available to the history recorder
- [x] #3 Sockets named in history are probed alongside the default socket
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add #{session_id} to the tmux list format and MuxSessionNode.
2. Probe status per backend/socket on GraphFragment/GraphSnapshot (mux_probes); probe named sockets from DiscoveryContext::mux_sockets; zellij exited sessions.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Discovery now carries tmux's `#{session_id}` (last format field, so older rows still parse) and reports `mux_probes` per backend and socket: listed, no server (a success), unavailable, or failed, plus zellij's exited sessions. `list_sessions_on` probes named tmux servers with `-L`, and `LocalDiscoveryConfig` feeds the sockets history records name into the probe set; sessions on a named server get a `<socket>:` native id so names can't collide. Verified by tmux and zellij unit tests (parser, per-socket probes, no-server is an empty success, missing binary is unavailable, a named socket is asked and namespaced) and a CLI test that kills a private tmux server and sees its mux go gone. Snapshot format is now 8. Full suite passes.
<!-- SECTION:FINAL_SUMMARY:END -->
