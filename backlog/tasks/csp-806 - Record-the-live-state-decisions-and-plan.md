---
id: CSP-806
title: Record the live-state decisions and plan
status: Done
assignee: []
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 01:50'
labels:
  - live-state
  - ls-model
milestone: m-26
dependencies: []
documentation:
  - docs/plans/live-state/README.md
  - docs/adr/0202-live-state-is-a-neutral-observation-with-a-closed-state-set.md
  - docs/adr/0203-the-live-state-store-folds-agent-events-per-session.md
  - >-
    docs/adr/0204-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md
  - >-
    docs/adr/0205-live-state-emitters-subscribe-to-a-minimal-event-set-per-harness.md
  - docs/adr/0206-live-state-sources-beyond-hooks-need-their-own-decision.md
priority: high
type: docs
ordinal: 892000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The research for live agent state produced decisions that every later story depends on: the state vocabulary, a store that stays off the daemon's hot path, a snapshot side table, the minimal event sets, and the rule that other sources need their own decisions. They have to be written down before code starts, in the repo's ADR style, so reviewers can accept or reverse them.

Scope: ADRs 0202 to 0206, the plan in `docs/plans/live-state/README.md` with six research notes, and the `docs/design.md` section and requirement. The ADR index lists them.

TUI changes: none.

Tests and checks: `git diff --check`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 ADRs 0202 to 0206 exist and are indexed in `docs/adr/README.md`
- [x] #2 `docs/design.md` has a Live Agent State section and requirement
- [x] #3 The plan lists the decisions taken without asking, with how to reverse each, and the decisions parked for the operator
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Wrote ADRs 0202 to 0206 (vocabulary, per-session store, `node_live` side table, minimal event sets and what a writer keeps, and the gate on hook-free sources), the plan with six research notes under `docs/plans/live-state/`, and the `docs/design.md` section and requirement. The plan lists eight decisions taken without asking, each with how to reverse it, and twelve decisions parked for the operator. Checked with `git diff --check`.
<!-- SECTION:FINAL_SUMMARY:END -->
