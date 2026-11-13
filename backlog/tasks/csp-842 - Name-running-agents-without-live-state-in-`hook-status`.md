---
id: CSP-842
title: Name running agents without live state in `hook status`
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 08:07'
updated_date: '2026-11-13 19:45'
labels:
  - live-state
  - ls-cli
milestone: m-26
dependencies: []
priority: low
ordinal: 927000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook status <harness>` ends with one live-state health line. It says `receiving` as soon as the live-state store holds any entry for the harness, so an agent whose hooks are silent (a workspace whose trust prompt was never accepted, an agent started before `hook init`) goes unnoticed while another agent of the same harness reports. Only when the store holds nothing for the harness does the line say `silent`. The evidence to tell the two apart already exists: the main-agent runtime processes in the saved graph and the pids the store records.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 With entries from one agent of a harness and another running agent of it that has no entry, the health line says how many running agents have no live state
- [x] #2 An agent whose entry records a different pid than its runtime process (a wrapper process) is not reported as silent when the session matches
- [x] #3 `hook status` still starts no process, asks no daemon, and stays as fast
- [x] #4 Tests cover the mixed case and the operations guide describes it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `running_agents` returns each running main-agent process of the harness in the saved graph with the native session ids the graph ties it to (its `ProcessIdentifiesSession` links, or its only `ProcessCandidatesSession` link), instead of a count.
2. An agent reports live state when a store entry of the harness records its pid, or names one of its sessions (so a wrapper whose entry records the inner pid is not silent when the session matches).
3. `Health::Receiving` carries the number of running agents that report none; the message appends `; N running agent(s) have no live state` when it is above zero. The label stays `receiving`; `silent` and the rest are unchanged. Counted only for a harness whose hooks report live state.
4. The saved graph is read lazily as before; in the receiving case it is now read once too (the same read the silent and pane-title cases do). No process, no daemon.
5. Tests: the mixed case (one agent with an entry, one without), the wrapper pid with a matching session, a sole candidate session, a non-reporting harness; docs/operations.md health section and CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
`running_agents` now returns each running main agent with the native session ids the saved graph ties it to (`ProcessIdentifiesSession` links, or the only `ProcessCandidatesSession` link, as the resolver reads them). An agent reports when a store entry of the harness records its pid or names one of those sessions. `Health::Receiving` gains `unreported`, appended to the message as `; N running agent(s) have/has no live state`; the label stays `receiving`. In the receiving case `hook status` now reads graph.bin once (the read the silent and pane-title cases already did); it still starts no process and asks no daemon (only graph.bin, the store file, and /proc through ProcProbe). Not matched: an entry whose pid differs and whose session the graph has not tied to the process yet (a brand-new session); filed as a follow-up.
Validation: run-some.sh --lib hook_health (12 passed); --test cli_smoke hook_status (6 passed, incl. the end-to-end mixed case); checks.sh exit 0; run-tests.sh exit 0, 5,647 passed, 0 failed, 1 ignored.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The `receiving` health line of `hook status <harness>` now counts the harness's running agents that no live-state entry describes and ends with `; 1 running agent has no live state` (or `N running agents have no live state`). An agent counts as reporting when an entry records its pid or names a session the saved graph ties its process to (its identifying link, or its only candidate), so a wrapper whose hook recorded the inner pid is not called silent once its session matches. The label and every other health stay as they were; the command still reads only the store, graph.bin, and /proc. Verified with unit tests (the mixed case, the wrapper pid with a matching session, a sole and an ambiguous candidate, a harness without live state) and an end-to-end `hook status claude-code` test with one reporting and one silent running agent; checks.sh and the full suite (5,647 passed). docs/operations.md (health table and troubleshooting) and CHANGELOG.md updated.
<!-- SECTION:FINAL_SUMMARY:END -->
