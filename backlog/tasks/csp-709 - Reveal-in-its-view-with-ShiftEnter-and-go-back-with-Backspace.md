---
id: CSP-709
title: 'Reveal in its view with Shift+Enter, and go back with Backspace'
status: To Do
assignee: []
created_date: '2026-10-27 02:16'
labels:
  - h-search
  - tui
milestone: m-11
dependencies: []
priority: medium
type: feature
ordinal: 795000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In the Search, History, and Pinned views the operator wants one key to jump to the native view of a selected session, mux, or PR, and a way back. Shift+Enter performs Reveal in its view for those entities; Backspace with the left pane focused returns to the view and selection from before the jump. Terminals often cannot send Shift+Enter, so it needs a fallback.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Shift+Enter in the Search, History, and Pinned views reveals the selected session, mux, or PR in its native view with its row selected, through the existing Reveal path; a documented fallback key works in terminals that cannot send Shift+Enter, and the . menu entry remains
- [ ] #2 Backspace with the left pane focused goes back to the previous view and selection, after Shift+Enter and after any cross-view jump from a reveal, quick select All result, or search; the right pane's Backspace is unchanged
- [ ] #3 A bounded back stack survives refreshes, drops entries whose view or row no longer exists, and is shown by a status hint; key tables, help, docs, and consistency tests are updated
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
