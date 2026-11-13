---
id: CSP-871
title: >-
  Show live state from claude agents --json for Claude Code sessions with no
  hook
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-13 05:22'
updated_date: '2026-11-13 21:39'
labels:
  - live-state
  - ls-policy
  - needs-decision
milestone: m-26
dependencies:
  - CSP-832
priority: low
ordinal: 951000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0218 runs `claude agents --json` only as a repair of a Claude Code hook entry stuck at `working` or `waiting`, and gives a session with no hook entry nothing: the poll runs only while some hook entry is due, so a hook-less session's state would appear by accident and then never age (`ready` and `waiting` hold while the process lives, ADR 0203). A Claude Code session whose hooks are untrusted, disabled (`disableAllHooks`, `--bare`, managed hooks only), or not installed therefore shows no live state, and `hook status` says `silent`.

Showing those sessions from the poll needs a cadence of its own (a regular poll whenever a Claude Code process with no hook entry is known, at some cost per call), a rule for how a poll-only state ages once polls stop, and a binding by pid to the Claude Code process discovery already knows. It is an operator decision under ADR 0207 decision 4 and needs its own ADR.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An ADR decides the cadence, the cost bound, how a poll-only state ages, and how it binds to a session or pane, and the operator approves it
- [x] #2 A Claude Code session with no hook entry shows the poll's state with source harness_poll, and loses it when polls stop, by tests with a fake runner and clock
- [x] #3 hook status reports how many Claude Code agents the poll shows, as it does for Codex pane titles
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Staleness story first; build only if it holds (it does, below). New ADR 0222 amending ADR 0218 decisions 2, 7, 10.
1. Cadence: the repair poller also runs (still at most once per 30 s, same processes: daemon cycles and a daemonless TUI's refresh worker; never one-shot commands or the overlay) while discovery knows a running Claude Code agent process with no hook entry: a main-agent RuntimeProcess of the harness whose pid runs now and that no store entry describes by pid or by a session the graph says it runs (hook status's "unreported" rule, moved to discovery and shared). Cost bound: one call (0.3 to 0.6 s) per 30 s while such an agent runs; none otherwise.
2. Binding: a listed agent binds poll-only only when it is such a known, unreported process (pid equal, start time equal when known), to the session the graph says that process runs, else to the live session node whose id the poll gives; only into an empty session slot; never pane-only. Agents with any hook entry keep ADR 0218's repair rules.
3. Aging: the binding records no pid, so its liveness is unknown and ADR 0203's rules age it with no heartbeat: working quiet after 10 minutes and every state unknown/stale 15 minutes after the last poll that listed it. It drops at once when the process ends (the bind checks the listed pid) or a newer poll no longer lists it. So it cannot appear by accident (polls run exactly while such an agent is known) or hold forever.
4. hook status adds "N agents shown from the agent list" from the saved graph's harness_poll bindings.
5. Tests with a fake runner and clock (poller cadence for unreported agents; bind, drop on gone pid, drop when unlisted, quiet/stale when polls stop), unreported-agent detection, hook status count. Docs: ADR 0222, ADR 0218 note, index, operations, design, CHANGELOG.
AC #1 asks for the operator's approval of the ADR: leave it unchecked and the task not Done, label needs-decision.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (commit 8886891b plus the docs commit after it) with ADR 0222, marked "Accepted, awaiting the operator's approval". AC #1 is left unchecked and the task not Done because it asks for the operator's approval of the ADR, which no agent can give.

Staleness story (the reason it was built): the poll runs at most every 30 s and only while a running Claude Code main agent known to discovery has no hook entry (or a hook entry is due, as before); a poll-only state binds only for such a known agent, so nothing appears because another agent made the poller run; it records no pid, so ADR 0203 ages it with no heartbeat (working quiet after 10 min, any state unknown/stale 15 min after the last poll that listed it); it drops at once when its pid is gone or changed, or a newer poll no longer lists it.

Choices for the operator: (1) approve ADR 0222 as is, then check AC #1, remove needs-decision, and set Done; (2) reject it: revert the CSP-871 code and docs commits (ADR 0218 decision 7 stands again); (3) adjust: e.g. bind by session id without a known process (rejected in the ADR as the "accident" case), or a separate opt-in switch for the hook-less poll (today only CONSPECTUS_DISABLE_LIVE_POLL turns all polling off).

Validation: new live_poll_only_tests (binding by process and by session id, unknown/background/subagent/no-pid agents get nothing, hook entries keep the hook rules, gone/changed pid and unlisted agents drop, cadence every 30 s with a scripted runner and clock, quiet then stale when polls fail, nothing without a poller, sentinel through JSON/Debug/graph.bin), a poller cadence test, and hook status count/note tests. checks.sh green; full suite 5,680 passed, 0 failed (before a test rename and the sentinel test, which pass alone).
<!-- SECTION:NOTES:END -->
