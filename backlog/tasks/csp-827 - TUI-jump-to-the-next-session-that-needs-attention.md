---
id: CSP-827
title: 'TUI: jump to the next session that needs attention'
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
ordinal: 913000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The cheapest high-value control in the surveyed tools is a key that moves to the next waiting agent from any view, then attach (the existing gesture). The order (longest-waiting first or most recent first) and whether it includes finished-unseen turns are the operator's.

Scope: Per the operator's decision: a key in the scoped keymap with a generated hint, the order, and the wrap rule.

TUI changes: A new key and status hint.

Tests and checks: Keymap tests, generated help, cursor movement across views.

Blockers: Operator decisions 5 and 10, and the cell story.
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
