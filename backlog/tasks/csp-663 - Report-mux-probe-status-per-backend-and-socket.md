---
id: CSP-663
title: Report mux probe status per backend and socket
status: To Do
assignee: []
created_date: '2026-10-22 01:10'
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
- [ ] #1 Discovered tmux muxes carry tmux's `session_id`
- [ ] #2 Each probed backend and socket reports success, empty, or failure, available to the history recorder
- [ ] #3 Sockets named in history are probed alongside the default socket
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
