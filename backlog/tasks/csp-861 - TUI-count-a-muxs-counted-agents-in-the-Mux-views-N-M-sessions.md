---
id: CSP-861
title: 'TUI: count a mux''s counted agents in the Mux view''s N/M sessions'
status: To Do
assignee: []
created_date: '2026-11-13 02:51'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 946000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: under a narrowing filter the Mux view's header reads `N/M sessions` from the sessions the listed muxes attach by a resolved link (`attached_count`). A mux listed for the agents its mark counts (an ambiguous session whose preferred mux it is, an agent in a pane no session matches) adds none, so `agent:waiting` showing only the showcase's `ambiguous` mux reads `0/11 sessions` beside a mux whose mark says two agents wait (ADR 0215, consequence 2). Decide whether N should count the sessions a mux's live counts hold, or say why it does not.
- Tests: header count under `agent:waiting` in the Mux view over a world with an ambiguous waiting session; the count under the other filters unchanged.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
