---
id: CSP-779
title: Skip the scrollbar gutter where its track cannot draw
status: To Do
assignee: []
created_date: '2026-10-29 09:42'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 865000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`tuikit::scroll::ScrollGutter::layout` carves a 1-column gutter whenever the rows overflow, but `ScrollGutter::render` keeps a row clear above and below the track and ratatui draws nothing once the track has no room between its arrows, so a scrolled body shorter than five rows keeps a blank column and shows no bar. The action menu at 40x10 is the visible case (its entries get four rows, so labels lose a column for nothing); the help page, the panes, and the preview can hit it on very short terminals.

Scope: make the gutter fade when its track cannot draw (no gutter below the height that leaves at least one track cell), in `ScrollGutter::layout`, so every scrolled body gets the column back. Update the scroll page and the action menu page's seam.

Tests and checks: gutter tests at heights 3 to 6; the action menu's goldens and frames at 40x10; `--snapshot` of the help page and a pane at a very short height.

Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
