---
id: CSP-873
title: Match running agents to live-state entries by tmux pane in hook status
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 19:46'
updated_date: '2026-11-14 01:51'
labels:
  - live-state
  - ls-cli
milestone: m-26
dependencies: []
priority: low
ordinal: 953000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook status` counts a running agent as reporting when a live-state entry records its pid or names a session the saved graph ties its process to (CSP-842). An agent whose entry records a different pid (a wrapper) and whose session the graph has not tied to the process yet, such as a new session with no transcript, is still counted as having no live state, though its entry names the tmux socket, session, and pane it runs in.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 An agent whose entry names its tmux pane is not counted as having no live state, whatever pid the entry recorded
- [x] #2 Two agents in panes of one mux are still told apart
- [x] #3 `hook status` (and the poll-only rule that shares it) still starts no process and reads only the store, graph.bin, and /proc; its time is measured before and after
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. RunningAgent (src/discovery/live_poll.rs) gains the tmux panes the saved graph saw its process in: for each mux containment link observed from the mux's active pane (process tree or the pane's files), that mux's active pane as its probe's pane listing names it, with the server's socket name and probed socket path.
2. `reported_by(entry, probe)` also holds when the entry names that pane (server by the binding's rule, session, pane id) and the entry has not exited and its process is not found dead, so an exited agent's entry in the same pane does not speak for the agent that replaced it. Health::assess takes the probe; `unreported` (the poll-only rule of ADR 0222, which shares the rule) passes its probe.
3. Tests: hook_health_tests (pane entry with another pid reports; an entry naming another pane of the same mux, or the same pane on another server, does not; an exited or dead entry in the pane does not), cli_smoke end to end.
4. Measure `hook status claude-code` before and after on a large scratch graph (wall time, strace for no exec).
5. Docs: operations.md health text, CHANGELOG, ADR 0222 amendment for the shared rule.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
The pane comes from mux containment links observed under the mux's active pane (match kinds active_pane_process_observation and the active-pane fd/command kinds), resolved to the pane the probe lists as that session's active one. A hook's own process observation keeps no pane; its entry matches by pid. A pane match needs a live entry (not exited, process not found dead), so an exited agent's entry does not speak for its successor in the pane. The rule is ADR 0222's shared unreported rule, so the poll-only binding follows it too (amended ADR 0222).

Measured on a scratch graph of 300 running agents, each under the active pane of its own tmux session, with 300 live entries naming those panes by other pids (debug build, 30 runs each, twice): before 78/75 ms, reporting all 300 as having no live state; after 85/93 ms, reporting none. The added cost is one /proc stat read per pane-matched entry. strace -f: only the binary's own execve, no clone/fork, before and after.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`RunningAgent` (src/discovery/live_poll.rs) now carries the tmux panes the saved graph saw its process in: for each mux that found it under its active pane, the pane the mux's probe lists as that session's active one, with the server's socket name and probed path. `reported_by(entry, probe)` also holds when the entry names one of those panes (server by the binding's socket-path rule, session, pane id) and the entry has neither exited nor lost its process. `hook status` (Health::assess takes the probe) and the poll's unreported agents (ADR 0222's shared rule) both use it.

Tests: hook_health_tests (pane entry with another pid reports; another pane of the same mux, the same pane on another server, an exited or dead entry, and an entry missing session or pane do not; the active pane is named only for active-pane observations, not hook ones), live_poll_only_tests (the poller's unreported list follows the pane rule), cli_smoke end to end through graph.bin and the store (an entry from pane %2 leaves the %1 agent counted; from %1 it reports). Measured `hook status claude-code`: 300 agents and 300 pane entries, debug build, ~77 ms before vs ~89 ms after; strace shows no process started. ADR 0222 amended; operations.md, CHANGELOG. `checks.sh` and `just check` (5703 passed) green.
<!-- SECTION:FINAL_SUMMARY:END -->
