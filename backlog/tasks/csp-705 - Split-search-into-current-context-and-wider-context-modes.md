---
id: CSP-705
title: Split search into current-context and wider-context modes
status: To Do
assignee: []
created_date: '2026-10-26 13:48'
labels:
  - h-search
  - tui
milestone: m-11
dependencies: []
priority: high
type: feature
ordinal: 784000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Operator redesign of search (2026-10-26). / becomes a quick selector over the current context (the rows as filtered), with Tab widening it to every entity; Ctrl+/ becomes content search over transcripts and mux scrollback, with Tab widening from the current context to all sessions and muxes. Content searches are remembered and paged in the Search view. The search modal uses far more of the terminal width. This replaces the contents scope inside / that shipped in CSP-616.06.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
