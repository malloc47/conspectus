---
id: CSP-585
title: Give each node kind its own Preview
status: Done
assignee:
  - '@claude'
created_date: '2026-10-05 04:22'
updated_date: '2026-10-12 16:21'
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
- [x] #1 Every node kind previews what the design subtask's ADR assigns it, or shows nothing where the ADR says so
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Every node kind now previews what ADR 0112 assigns it: sessions their transcript through the native viewer (pane one remembered choice away), mux sessions their pane, pins through their binding, checkouts `git status` over a listing, repos their history and remotes, workspaces a listing, branches their commits with ahead/behind, PRs their title, checks, reviews, and body via `gh`, processes their `/proc` tree, and forks only their fields. Every field row previews by value type, and Related rows show the neighbor's preview below their evidence. All of it runs through one read-only, memory-only preview mechanism (`CSP-585.02`–`.09`), verified with `just check` and live `tui --snapshot` checks. Follow-ups: `CSP-587` (showcase transcripts for the README frame), `CSP-588` (search by displayed session id), and closing `CSP-171.01`.
<!-- SECTION:FINAL_SUMMARY:END -->
