---
id: CSP-826
title: 'TUI: order, group, and filter by live state'
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 20:05'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies:
  - CSP-823
documentation:
  - docs/plans/live-state/tui-options.md
priority: low
type: enhancement
ordinal: 912000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Operators of other tools sort or group by "needs me first". Rows that re-sort under the cursor are disorienting, and Conspectus keeps rows still on fold, so whether this is a sort control, a "state" grouping in the cycle, a filter, or a dedicated attention view needs a decision.

Scope: Per the operator's decision: sort and grouping options, a `--state` filter in the CLI, a Controls row, filter chips with counts, and a stability rule so the cursor and the operator's place do not jump.

TUI changes: Controls, sort, and grouping gain options as decided.

Tests and checks: Sort and filter tests; cursor stability under live changes; snapshot frames.

Blockers: Operator decision 4 and the cell story.
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 4: a float-waiting-first control (off by default, the same mechanism as float-sessions-with-a-mux), a State filter row in Controls, and --state on conspectus table. Rows never re-sort under the cursor between operator actions. No state grouping and no Attention view in this story. Note from CSP-814: sorting or filtering by live state means the tree must be rebuilt when the overlay changes, instead of refilling cells in place.
<!-- SECTION:NOTES:END -->
