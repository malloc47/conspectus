---
id: CSP-871
title: >-
  Show live state from claude agents --json for Claude Code sessions with no
  hook
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 05:22'
updated_date: '2026-11-14 05:38'
labels:
  - live-state
  - ls-policy
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
- [x] #1 An ADR decides the cadence, the cost bound, how a poll-only state ages, and how it binds to a session or pane, and records the operator's decision (2026-11-13): kept as an opt-in, off by default (`[server] live_poll_without_hooks`), with ADR 0218's repair unchanged and on by default
- [x] #2 With the setting on, a Claude Code session with no hook entry shows the poll's state with source harness_poll, and loses it when polls stop, by tests with a fake runner and clock
- [x] #3 With the setting off (the default), a hook-less agent starts no poll and gets no poll-only observation, in the discovery pass and the TUI's overlay, while the repair poll behaves as before; `CONSPECTUS_DISABLE_LIVE_POLL` builds no poller whatever the setting says
- [x] #4 `hook status` reports how many Claude Code agents the poll shows only with the setting on, as it does for Codex pane titles; with it off it counts none and, when hook-less Claude Code agents run, names the setting and how to turn it on
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

Operator decision 2026-11-13: keep the code, but make the hook-less poll opt-in, off by default; the repair poll of ADR 0218 is unchanged and stays on.
6. Config key `[server] live_poll_without_hooks` (bool, default false), parsed into a `PollScope` (`Repair` | `WithoutHooks`) on `ServerConfig`; a wrong type is a diagnostic and keeps the default. No environment variable turns it on; `CONSPECTUS_DISABLE_LIVE_POLL` still builds no poller whatever the key says.
7. The scope rides on each `LivePoller` (`from_env(scope)`): a `Repair` poller ignores unreported agents, so no poll runs for them; `apply_live_state` counts unreported agents only for a `WithoutHooks` poller and binds poll-only agents only then (`bind_live_scoped`); `bind_live` keeps the default (repair only). The TUI's overlay binds with the TUI's scope; serve and the TUI read it from config.
8. `hook status` counts agents shown from the agent list only with the key on; with it off, when running agents have no entry, it says which setting shows them from the agent list.
9. Tests: config parse/default/diagnostic; poller does not run for unreported agents in `Repair`; poll-only binding is empty in `Repair` and unchanged in `WithoutHooks`; the overlay; kill switch over the key (pure pollers fn plus a daemon test with the key on in a scratch config); hook status note/hint text.
10. Docs: ADR 0222 status and amendment, ADR 0218 note, ADR index, operations (how to turn it on), CHANGELOG, design.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented (commit 8886891b plus the docs commit after it) with ADR 0222, marked "Accepted, awaiting the operator's approval". AC #1 is left unchecked and the task not Done because it asks for the operator's approval of the ADR, which no agent can give.

Staleness story (the reason it was built): the poll runs at most every 30 s and only while a running Claude Code main agent known to discovery has no hook entry (or a hook entry is due, as before); a poll-only state binds only for such a known agent, so nothing appears because another agent made the poller run; it records no pid, so ADR 0203 ages it with no heartbeat (working quiet after 10 min, any state unknown/stale 15 min after the last poll that listed it); it drops at once when its pid is gone or changed, or a newer poll no longer lists it.

Choices for the operator: (1) approve ADR 0222 as is, then check AC #1, remove needs-decision, and set Done; (2) reject it: revert the CSP-871 code and docs commits (ADR 0218 decision 7 stands again); (3) adjust: e.g. bind by session id without a known process (rejected in the ADR as the "accident" case), or a separate opt-in switch for the hook-less poll (today only CONSPECTUS_DISABLE_LIVE_POLL turns all polling off).

Validation: new live_poll_only_tests (binding by process and by session id, unknown/background/subagent/no-pid agents get nothing, hook entries keep the hook rules, gone/changed pid and unlisted agents drop, cadence every 30 s with a scripted runner and clock, quiet then stale when polls fail, nothing without a poller, sentinel through JSON/Debug/graph.bin), a poller cadence test, and hook status count/note tests. checks.sh green; full suite 5,680 passed, 0 failed (before a test rename and the sentinel test, which pass alone).

Operator decision 2026-11-13: keep the hook-less poll, but opt-in and off by default; the repair poll of ADR 0218 stays as it was and on. Implemented in bdaf0ce1 (code and tests) and 41564bfd (docs): `[server] live_poll_without_hooks` (bool, default false) parsed into `PollScope` (`Repair` | `WithoutHooks`); each poller carries the scope (`live_poller::from_env(scope)`), a `Repair` poller ignores unreported agents, `apply_live_state` counts them and binds poll-only agents only for a `WithoutHooks` poller (`bind_live_scoped`; `bind_live` keeps the default), and the TUI overlay binds with the TUI's configured scope. No environment variable turns it on; `CONSPECTUS_DISABLE_LIVE_POLL` builds no poller whatever the setting says. `hook status` counts agents shown from the agent list only with the setting on; off, it ends the line with "N agent(s) could be shown from its/their agent list: set `live_poll_without_hooks = true` under `[server]` in the config file" when running agents have no live state.
Validation: checks.sh green; full cargo test 5,716 passed, 0 failed; `just check` (nextest, doctests, rustdoc, framework matrix) exit 0, 5,716 passed, 1 skipped.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Kept the hook-less part of the `claude agents --json` poll (ADR 0222) and made it opt-in, off by default, as the operator decided on 2026-11-13. The poll starts the `claude` first on PATH, often an operator's wrapper, every 30 s, so running it on a timer is the operator's call. The repair of stuck hook entries (ADR 0218) is unchanged and on by default.

- Setting: `[server] live_poll_without_hooks = true` (default false; a value that is not a boolean is a config diagnostic and leaves it off). `conspectus serve` and a daemonless TUI read it when they start. No environment variable turns it on, and `CONSPECTUS_DISABLE_LIVE_POLL` still builds no poller whatever it says.
- Off: a hook-less Claude Code agent starts no poll and gets no poll-only state, in the discovery pass or the TUI's overlay. `hook status` counts no agents shown from the agent list, and when hook-less agents run it ends the health line with "1 agent could be shown from its agent list: set `live_poll_without_hooks = true` under `[server]` in the config file".
- On: the behavior is the same as before.
- Docs: ADR 0222 (status, plus a second amendment giving the reason and the rules), the ADR 0218 note, the ADR index, operations (how to turn it on, the config reference, health and troubleshooting), design, and CHANGELOG.

Visible change: by default, a Claude Code agent with no hooks shows no live state again, as before CSP-871. That is the intended change; no layout changed.

Verified by tests:
- config default, true/false, project over user, and a bad value;
- the poller runs nothing for unreported agents in `Repair` and still repairs a stuck entry;
- the kill switch builds no poller in either scope;
- with a fake runner and clock, poll-only binding and the pass are empty with the setting off and unchanged with it on (sentinel test still green);
- the overlay shows the poll-only state only when configured;
- `hook status` note and hint unit tests, and smoke tests through the binary with the setting on and off;
- a daemon test where the kill switch wins over the setting turned on in a scratch config.

checks.sh green; full cargo test 5,716 passed; `just check` exit 0.
<!-- SECTION:FINAL_SUMMARY:END -->
