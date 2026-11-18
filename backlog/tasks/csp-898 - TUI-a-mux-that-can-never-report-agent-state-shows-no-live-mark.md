---
id: CSP-898
title: 'TUI: a mux that can never report agent state shows no live mark'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-18 00:43'
updated_date: '2026-11-18 00:56'
labels:
  - tui
dependencies: []
ordinal: 974000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A mux with no agent harness and no session linkage (the daemon's own conspectus-server, a plain shell) draws the no-state mark in its live cell, as if its state were unknown or its hooks off. It can never have reportable state, so the mark is noise; the operator asked for no symbol at all there.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A mux row whose pane runs no agent harness and that no session or agent counts toward draws a blank live cell, in the Mux view (running and stopped rows alike), with the column grid unchanged
- [ ] #2 A mux that has an agent, a harness program in a pane, or any live evidence keeps its mark, the no-state mark included
- [ ] #3 A session row's live cell is unchanged
- [ ] #4 docs/tui-columns.md and the help legend say when the no-state mark shows; tests cover both cases
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
