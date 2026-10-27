---
id: CSP-708
title: Open pane results in a full-screen pane viewer with an attach shortcut
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
ordinal: 794000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Selecting a mux pane result in content search opens a small scrollback pager. It should be a full-screen viewer like the session viewer, positioned at the match, with a shortcut to attach to the mux.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pane result opens a full-screen viewer of the pane scrollback, scrolled to the match with the terms highlighted, styled and keyed like the session viewer (find, n/N, help, q to quit, Esc steps back)
- [ ] #2 A documented key in the viewer attaches to the mux, suspending the TUI as the normal attach does, and refuses with a message when the mux is stopped
- [ ] #3 The viewer's help is generated from its key table, and layout, key-convention, and text-hygiene tests cover it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
