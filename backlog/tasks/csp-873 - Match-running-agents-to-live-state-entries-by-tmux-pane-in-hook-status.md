---
id: CSP-873
title: Match running agents to live-state entries by tmux pane in hook status
status: To Do
assignee: []
created_date: '2026-11-13 19:46'
labels:
  - live-state
  - ls-cli
milestone: m-26
dependencies: []
priority: low
ordinal: 953000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`hook status` counts a running agent as reporting when a live-state entry records its pid or names a session the saved graph ties its process to (CSP-842). An agent whose entry records a different pid (a wrapper) and whose session the graph has not tied to the process yet, such as a new session with no transcript, is still counted as having no live state, though its entry names the tmux socket, session, and pane it runs in.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An agent whose entry names its tmux pane is not counted as having no live state, whatever pid the entry recorded
- [ ] #2 Two agents in panes of one mux are still told apart
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
