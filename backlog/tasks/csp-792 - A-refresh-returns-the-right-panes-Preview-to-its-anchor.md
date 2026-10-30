---
id: CSP-792
title: A refresh returns the right pane's Preview to its anchor
status: To Do
assignee: []
created_date: '2026-10-30 08:49'
labels:
  - tui-framework
dependencies: []
priority: low
type: bug
ordinal: 878000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while fixing CSP-789. `App::recompute_explorer` calls `reset_preview_scroll` before anything else, and every `SetData` (the periodic refresh, `r`) and every projection rebuild (`SetTree`: filter and sort changes) runs it. So an operator who has scrolled the Preview with `J`/`K` (up a transcript from its newest turns, or down a pane capture or a PR body) is put back at the anchor on the next refresh, even when the left selection, the explorer's focused node, and its cursor did not change. `App::update` already resets the Preview when what it is pointed at changes (`preview_focus`), which suggests the unconditional reset predates that rule rather than being intended. Not yet confirmed with a reducer test; confirm first, then decide against ADR 0181's anchoring rules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A reducer test scrolls the Preview, delivers a refresh with the same snapshot and selection, and finds the scroll kept; or the task records why the reset is intended and closes
- [ ] #2 A refresh that changes what the Preview shows (another selection, focused node, or cursor row) still starts it at its anchor
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
