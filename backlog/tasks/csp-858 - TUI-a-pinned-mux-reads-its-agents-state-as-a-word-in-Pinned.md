---
id: CSP-858
title: 'TUI: a pinned mux reads its agents state as a word in Pinned'
status: To Do
assignee: []
created_date: '2026-11-12 22:42'
updated_date: '2026-11-12 22:44'
labels:
  - live-state
  - ls-tui
  - needs-decision
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 943000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
In Pinned a running session pin reads `waiting`, `working`, `ready`, or `error` (ADR 0209 decision 4), but a mux pin keeps `live`, which the filled pin marker already says. ADR 0209 left a rolled-up word for a mux pin to the mux rollup work; CSP-824 carried out the operator's decision 2 for mux rows and group headers only, which does not cover Pinned words, and decision 7 (CSP-829) names session words. Whether a mux pin reads its agents' rolled-up state (by the same order as its row's mark) is a display choice for the operator.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pinned mux with agents in live states reads the word of its rolled-up mark in place of `live`, in that mark's style, or the operator's decision is recorded otherwise
- [ ] #2 One mux shows one state in its Mux view mark and its Pinned word; the consistency suite checks it
- [ ] #3 docs/tui-columns.md (state words) and an ADR describe it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
