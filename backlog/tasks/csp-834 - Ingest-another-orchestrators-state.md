---
id: CSP-834
title: Ingest another orchestrator's state
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 02:25'
labels:
  - live-state
  - ls-policy
  - needs-decision
milestone: m-26
dependencies:
  - CSP-812
documentation:
  - docs/adr/0207-live-state-sources-beyond-hooks-need-their-own-decision.md
  - docs/plans/live-state/research/05-orchestrators-and-ux.md
priority: low
type: enhancement
ordinal: 920000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Conspectus already opens agent-deck's `state.db`, which holds a status column. Reading it gives accurate state for sessions agent-deck manages at no detection cost, but the schema is third-party and covers only its own sessions.

Scope: An ADR, the adapter that reads the status column through the existing agent-deck reader, mapping to the neutral states, and the precedence against hook evidence.

TUI changes: Agent-deck sessions show state without hooks.

Tests and checks: Fixture database tests.

Blockers: Operator decision 11.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Pending the operator's decision
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
