---
id: CSP-214
title: Wire the widget into the right panel for un-muxed agent rows
status: To Do
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-transcript
milestone: m-11
dependencies:
  - CSP-208
  - CSP-209
  - CSP-210
  - CSP-213
ordinal: 202000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: route un-muxed `AgentSession` selections through the
  new widget instead of the single-line preview. Trigger the
  on-demand recent-turns read on selection change, similar to
  `CSP-168`'s mux capture and `CSP-171.01`'s `gh` enrichment:
  immediate placeholder while loading, then content. Cache
  by `AgentSessionId` for the lifetime of the TUI; invalidate
  on session-state mtime changes if cheap to detect. Muxed
  rows continue to render the mux capture preview from
  `CSP-168`. Mux candidate child rows continue to use the
  existing detail rendering.
- Tests: TUI integration tests for the un-muxed selection
  path, the muxed selection path (regression — mux capture
  still wins), the loading→content transition, and the
  cache-hit path on re-selection.
- Blockers: `CSP-208`, `CSP-209`,
  `CSP-210`, `CSP-213`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-010`
