---
id: CSP-706
title: Refine the two-level search from hands-on testing
status: To Do
assignee: []
created_date: '2026-10-26 22:10'
labels:
  - h-search
  - tui
milestone: m-11
dependencies: []
priority: high
type: feature
ordinal: 789000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Hands-on testing of CSP-705 found a cursor bug in quick select, and asked for: content search opening on All, the Search view staying fixed while the modal is open and the search persisted only when the modal exits, no duplicate entries in search history, granularity as a visible tab (and changeable from the Search view) instead of Ctrl-R, search details such as thinking behind their own modal, and q closing the viewer at once when it was opened from a search.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
