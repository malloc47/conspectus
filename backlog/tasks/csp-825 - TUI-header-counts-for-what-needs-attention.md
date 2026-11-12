---
id: CSP-825
title: 'TUI: header counts for what needs attention'
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
labels:
  - live-state
  - ls-tui
  - needs-decision
milestone: m-26
dependencies:
  - CSP-823
documentation:
  - docs/plans/live-state/tui-options.md
priority: low
type: enhancement
ordinal: 911000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
One glance at the header should answer "does anything need me?". What the chip counts (waiting only, or waiting plus ready-unseen plus working), whether it follows the filter, and where it ranks against freshness, counts, and the warning mark are open.

Scope: Per the operator's decision: the chip, its rank and give-way order (ADR 0190), and a key or click that filters to it.

TUI changes: The header gains a chip.

Tests and checks: Header snapshots at several widths; chip width rule tests (ADR 0157).

Blockers: Operator decision 3 and the cell story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Pending the operator's decisions
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
