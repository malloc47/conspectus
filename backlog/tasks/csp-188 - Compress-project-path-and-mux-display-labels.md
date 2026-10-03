---
id: CSP-188
title: 'Compress project, path, and mux display labels'
status: To Do
assignee: []
created_date: '2026-05-20 03:28'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-163
  - CSP-164
  - CSP-166
ordinal: 414000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: introduce display-label helpers for TUI rows and detail
  fields so raw paths and tmux native ids do not dominate prime
  screen space. Group rows should use a short project/checkout
  label first, with `~`-shortened path as dim secondary text
  when width allows. Mux fields should prefer a human-readable
  display name and keep the full native id available through the
  existing copy-id/node-show affordances or a dim overflow field.
- Tests: pure view-model tests for home-shortened paths,
  duplicate basename disambiguation, long tmux id compression,
  and stable labels across refresh; Ratatui snapshots for narrow
  and 120-col sessions views.
- Blockers: `CSP-163`, `CSP-164`, `CSP-166` v1 slices.
- **slice landed**: the sessions tree renders compact group
  primary labels with dim shortened-path secondary text, long mux
  labels are shortened in candidate rows and detail fields, and
  right-panel detail now receives `$HOME` for path shortening.
  Duplicate-basename disambiguation and shared view-model helpers
  remain open.
- **further slice from styling overhaul (Phase 7)**: group rows
  now carry agent + mux-state summary chips
  (`(N) ◉ a ◐ b ◯ c`) so the operator gets density without
  reading the path. Remaining work: duplicate-basename
  disambiguation and shared view-model helpers across CLI and
  TUI surfaces.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-012`
