---
id: CSP-860
title: See a turn that ended while you were attached to its mux
status: To Do
assignee: []
created_date: '2026-11-13 01:10'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
ordinal: 945000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0214 (CSP-828) reads a `ready` turn of an agent in a mux as seen once the mux's last attach time is at or after the turn's end, or a snapshot loaded after the turn ended lists a client attached. tmux moves `#{session_last_attached}` only when a client attaches or switches to the session, so a turn that ends while the operator watches it in an attached client is seen only if a refresh happens to list that client while it is still there (a timer refresh, every 30 seconds by default). When the operator returns to the TUI's pane sooner, the turn they just watched reads unseen (bold) until their next attach. Closing the gap needs to know which session the client was on when it came back (for example tmux's `#{client_last_session}` at the regain the hand-off already detects), which is a new tmux query and so a capability-manifest entry and the operator's call.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A turn that ends while a tmux client stays attached to its mux reads seen once the operator returns to the TUI from that mux, without waiting for another attach
- [ ] #2 A turn that ends after the client moved to another session still reads unseen
- [ ] #3 Any new tmux query is listed in docs/capabilities.toml with its ADR, and nothing is persisted
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
