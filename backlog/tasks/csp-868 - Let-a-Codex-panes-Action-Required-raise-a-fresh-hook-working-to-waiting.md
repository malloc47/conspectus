---
id: CSP-868
title: Let a Codex pane's Action Required raise a fresh hook working to waiting
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 04:56'
updated_date: '2026-11-14 14:24'
labels:
  - live-state
  - ls-policy
milestone: m-26
dependencies:
  - CSP-831
priority: low
ordinal: 948000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
With Codex hooks installed, a wait no hook reports (a question, an MCP elicitation, a sign-in prompt) reads `working` from the last tool or prompt event until that entry goes quiet ten minutes later, although the pane title already says `[ ! ] Action Required`. ADR 0217 lets the title fill in only where hook evidence is absent, quiet, or stale, plus one carve-out (an answered approval), because ADR 0207 decision 4 says a fresh hook record wins. Raising a fresh `working` to `waiting` on the title would be a second carve-out, so it is the operator's call.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Operator-approved 2026-11-13 and recorded as an amendment to ADR 0217 (a second carve-out to ADR 0207 decision 4): a fresh hook-derived working may turn waiting on the pane title
- [x] #2 The binding shows waiting/input (source pane_title) for a fresh hook working of a Codex pane once the title has read waiting on two consecutive probes, both after the hook entry's latest evidence
- [x] #3 A hook's ready, error, compacting, starting, and exited are never changed by a title; a quiet or stale working keeps giving way to the title as before
- [x] #4 Binding tests cover the raise, a single probe, a probe before the hook event, and each excluded state, at the rule and through the tables
- [x] #5 ADR, docs/operations.md, docs/design.md, and CHANGELOG.md describe it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. combine() in discovery/live_title.rs gains rule 5: a fresh hook entry reading working, with the title's waiting mark held on two consecutive probes after the entry's latest evidence, binds waiting/input from the pane title (since: the first probe of the run after the evidence, as for the answered-approval carve-out), carrying the hook entry's process, liveness, and subagents. Shared helper for the run's start.
2. Tests first at the rule (raise, single probe, probe before the hook event, each excluded state, dead process, quiet entry) and through bind_live (the session's binding and its mux's counts).
3. ADR amending ADR 0217 decision 7 and its consequences; operations.md, design.md, CHANGELOG, module docs.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision 2026-11-13: approved. Rule 5 added to ADR 0217's decision 7 by ADR 0232 (a second carve-out to ADR 0207 decision 4). Only a hook entry whose effective state is working with fresh confidence is raised, so a quiet or stale entry keeps giving way to the title under rule 3, and a dead process (effective exited) is never revived. Validation: live_title unit tests at the rule and through bind_live; `just check` green (5736 passed).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A fresh hook-derived `working` for a Codex pane now reads `waiting` (detail `input`, source `pane_title`) once the pane title has read the Action Required marker on two consecutive probes, both after the hook entry's latest evidence (ADR 0232, amending ADR 0217 decision 7, a second carve-out to ADR 0207 decision 4 by the operator's decision). `combine()` in `src/discovery/live_title.rs` gained rule 5 beside the answered-approval rule, sharing a helper for the run's start; the hook entry's process, liveness, and subagent count carry over and the store is not rewritten. Ready, error, compacting, starting, exited (and a working whose process was found gone) are never changed; a single probe, a probe before or at the hook event, and the next hook event leave or restore the hook's working. This closes the ten-minute gap for MCP elicitations, sign-in prompts, and questions. Docs: ADR 0232 (indexed, linked from 0207 and 0217), operations.md, design.md, CHANGELOG. Verified with binding tests for the raise, a single probe, a probe before the hook event, each excluded state, the run's start, and the tables (`bind_live`), and `just check` (5736 passed). Visible change: a Codex waiting on a question shows `!` after about ten seconds instead of ten minutes; no static frame changes.
<!-- SECTION:FINAL_SUMMARY:END -->
