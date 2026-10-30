---
id: CSP-786
title: Keep the left pane's row width when its list overflows
status: To Do
assignee: []
created_date: '2026-10-30 05:19'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 872000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`TreeView` carves the scrollbar gutter out of the pane only while the rows overflow it, so every row is laid out one cell narrower once the list grows past the pane. Columns whose width steps with the room or the row (the mux name, the Search view's hit tag) or takes a share of it can then jump by several cells: at 160 columns the Search view's rows are 78 cells while they fit and 77 once they overflow, which takes a hit's tag from 20 cells to 14 (`turn 46 · mes…`). The search palettes avoid this by reserving the scrollbar column (`Palette::reserve_scrollbar`, ADR 0175 decision 4). Decide whether the tree reserves the column too (rows always end a cell before the border) or lays its stepped and shared columns out from the pane's whole inner width, and record the choice.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A row of the left pane is laid out at one width whether or not its list overflows, or the chosen rule is recorded with the reason
- [ ] #2 The Search view's hit rows at 160 columns keep one tag width as results stream past the pane
- [ ] #3 Before/after --snapshot frames of every view at 160, 110, 80, 60, and 40 columns show only the intended changes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
