---
id: CSP-833
title: Live state from transcript and rollout tails
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 20:06'
labels:
  - live-state
  - ls-policy
  - needs-decision
milestone: m-26
dependencies:
  - CSP-812
documentation:
  - docs/adr/0207-live-state-sources-beyond-hooks-need-their-own-decision.md
priority: low
type: enhancement
ordinal: 919000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Without hooks a session's last transcript entries still say whether a turn is generating, running a tool, or finished, with medium confidence. They cannot show waiting, and the formats change between harness versions.

Scope: An ADR stating the ADR 0086 tier (structural fields only, in memory), per-harness tail parsers behind the adapter, the confidence they carry, and how they yield to hook evidence.

TUI changes: Sessions without hooks show a coarse working or ready state.

Tests and checks: Tail fixtures per harness; parser tolerance tests.

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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 11: not approved for now. Leave this story alone unless the operator asks.
<!-- SECTION:NOTES:END -->
