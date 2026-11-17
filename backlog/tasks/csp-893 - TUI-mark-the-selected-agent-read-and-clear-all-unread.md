---
id: CSP-893
title: 'TUI: mark the selected agent read, and clear all unread'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-17 01:02'
updated_date: '2026-11-17 01:46'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies:
  - CSP-892
priority: medium
ordinal: 969000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The gestures for the unread marks of the task before this one: the operator wants to mark one agent read and to clear every unread mark, and reattaching to an agent already reads it (attach evidence). The keys are `c` (read the selected row's agents) and `C` (clear all unread), free in the key tables, and both are in the row action menu, whichever view opened it, by the repo's rule that a gesture and a menu agree.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 c marks every agent on the selected row read (a session row: its agent; a mux row: the agents counted on it; a pin row: the pinned entity's) and C clears every unread mark, in every view, with a toast saying how many; both are bindings of the key tables, so the footers, status hints, and help list them with no hand-written text
- [x] #2 The row action menu lists Mark read for a row with an unread agent and Clear all unread whenever any is unread, by the same code as the keys, and the consistency tests assert the same gesture does the same thing in every view
- [x] #3 Marking read saves through the existing persist effect and declares its lifecycle in tui::effect::lifecycle; undo is not offered (reading is not undoable), and the rows update in place with no refresh
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
