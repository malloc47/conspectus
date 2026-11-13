---
id: CSP-868
title: Let a Codex pane's Action Required raise a fresh hook working to waiting
status: To Do
assignee: []
created_date: '2026-11-13 04:56'
labels:
  - live-state
  - ls-policy
  - needs-decision
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
- [ ] #1 The operator decides whether a fresh hook-derived working may turn waiting on the title, and the decision is recorded as an amendment to ADR 0217
- [ ] #2 If approved, the binding shows waiting/input for a fresh hook working once the title has read waiting on two consecutive probes after the hook entry's latest evidence, and never for ready, error, compacting, or exited
- [ ] #3 Binding tests cover the raise, a single probe, and a probe before the hook event
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
