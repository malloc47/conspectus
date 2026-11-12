---
id: CSP-830
title: 'TUI: notify when an agent needs you'
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 20:06'
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
ordinal: 916000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The harnesses already notify, so doubling can be noise. The options are nothing, a bell or terminal notification from the open TUI while unfocused, or a desktop notification (a new subprocess and capability).

Scope: Per the operator's decision, including per-state toggles and rate limits.

TUI changes: Depends on the choice.

Tests and checks: Notification tests; capability manifest entry if a program is launched.

Blockers: Operator decision 9 and the cell story.
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
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 9: no notifications in v1, because the harnesses already notify. Deferred, not decided against; revisit if the operator asks (a terminal bell while the TUI is open and unfocused, per-state toggles). Do not start this story.
<!-- SECTION:NOTES:END -->
