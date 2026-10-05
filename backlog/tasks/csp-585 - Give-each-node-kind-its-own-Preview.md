---
id: CSP-585
title: Give each node kind its own Preview
status: To Do
assignee: []
created_date: '2026-10-05 04:22'
labels:
  - h-preview
milestone: m-11
dependencies: []
ordinal: 625000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: the Preview area is built around tmux. Its store caches `tmux capture-pane` output keyed by mux session, so selecting an agent session shows the pane it happens to run in, and most other node kinds have nothing to preview. For a session, what identifies it is the conversation, not the terminal around it.
- Direction (operator, 2026-10-05): each node kind exposes its own preview when it has something worth showing, instead of borrowing the mux capture. An agent session previews its transcript through an inline version of the embedded session viewer (ADR 0052). A directory-backed value, such as a checkout or a session's working directory, could list the directory's contents.
- Related: `CSP-213`, `CSP-214`, and `CSP-171.03` were filed before the native viewer existed. They add a separate transcript widget for un-muxed sessions only and keep the mux capture for muxed ones, which this story reverses. `CSP-185` and `CSP-192` refine the mux capture itself. ADR 0078 (surface division of labor) and ADR 0106 (preview wrap modes) govern the pane today.
- Follow-ups: once the design subtask settles the per-node mapping, file one implementation subtask for each node kind that gains a preview.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Every node kind previews what the design subtask's ADR assigns it, or shows nothing where the ADR says so
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
