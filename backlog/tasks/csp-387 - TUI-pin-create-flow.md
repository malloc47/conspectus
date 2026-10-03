---
id: CSP-387
title: TUI pin create flow
status: Done
assignee: []
created_date: '2026-06-05 12:25'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-369
  - CSP-377
  - CSP-253
ordinal: 312000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: make the Controls overlay Pins group capable of creating
  pins without dropping to the CLI. Reuse the CSP-369 mutation
  helper and ADR 0030 text input primitive. Fields: `id`,
  `display_name`, `harness`, `cwd`, `mux.name`, optional
  `mux.socket_name`, optional launch argv override, and store
  (`auto` / `project` / `user`). Defaults should come from the
  current selection where possible: selected session gives harness
  + cwd + display candidate; selected checkout/repo gives cwd;
  otherwise cwd starts blank. Validation mirrors CLI create and
  never writes until the confirmation step succeeds.
- Tests: reducer tests for field editing, defaults from session
  and checkout selections, validation failures, cancel-no-write,
  and successful create through the shared write helper. Snapshot
  tests for the create overlay and validation messages.
- Blockers: `CSP-369`, `CSP-377`, `CSP-253`.
- Delivered: Controls overlay `Pins > create` opens a
  multi-field create modal, seeds fields from the selected session
  or graph group where possible, validates required fields before
  dispatch, and routes successful creates through the shared pin
  write helper with `auto` / `project` / `user` store selection.
  Static scenario TUIs keep mutation disabled and surface a status
  message instead of writing.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PIN-022`
