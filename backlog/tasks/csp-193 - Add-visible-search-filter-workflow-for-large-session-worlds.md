---
id: CSP-193
title: Add visible search/filter workflow for large session worlds
status: To Do
assignee: []
created_date: '2026-05-20 03:28'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-163
  - CSP-165
ordinal: 443000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: finish the `/` in-view search overlay for the TUI and
  make active filtering visible in the header or status bar.
  Matching should cover project label, path, harness, short id,
  session title, preview snippet, branch/PR labels when present,
  and mux display label. Results should preserve enough group
  context that the operator understands where a matched session
  lives.
- Tests: matcher tests for each searchable field; reducer tests
  for open/type/clear/accept/cancel; Ratatui snapshots for an
  active query, zero results, and grouped result context.
- Blockers: `CSP-163`, `CSP-165`; adding a heavyweight matcher
  still requires following ADR 0024's dependency policy.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-017`
