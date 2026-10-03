---
id: CSP-239
title: Mux lockstep helper
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies:
  - CSP-232
ordinal: 288000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: pure function consumed by the CLI rename command and the TUI
  rename action. Given a target node, the current snapshot, and a
  `--no-mux` flag, returns a `RenamePlan { agent_alias_write,
  mux_native_rename }`. Refuses lockstep with a typed reason when the
  target has ambiguous `LinkedToMux` candidates (per ADR 0029 lockstep
  rule) — operator must either resolve ambiguity first or pass
  `--no-mux`.
- Tests: unit tests across resolved-single-mux, ambiguous-mux,
  no-mux-link, and `--no-mux`-flag cases.
- Blockers: `CSP-232`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-009`
