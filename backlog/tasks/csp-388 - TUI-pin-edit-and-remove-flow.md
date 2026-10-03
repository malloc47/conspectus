---
id: CSP-388
title: TUI pin edit and remove flow
status: Done
assignee: []
created_date: '2026-06-05 12:25'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-369
  - CSP-374
  - CSP-377
  - CSP-253
ordinal: 313000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: bring existing pins to CRUD parity with CLI
  `pin rename` / `pin rm` from the Controls overlay, while keeping
  the row-level `R` and `Delete` accelerators from CSP-377.
  Edit supports id changes, display-name changes, mux-name changes
  when the operator explicitly chooses rebind semantics, optional
  socket-name changes, launch argv edits, and store/path display so
  the user can see which TOML file will be touched. Remove uses a
  confirmation modal that names the pin id, display name, and store
  path before invoking the shared remove helper.
- Tests: reducer tests for edit confirmation, cancel, duplicate-id
  rejection, duplicate-mux rejection, lockstep rename handoff, and
  remove confirmation. Snapshot tests for edit and delete states.
- Blockers: `CSP-369`, `CSP-374`, `CSP-377`, `CSP-253`.
- Delivered: Controls overlay `Pins > rename` opens an edit modal
  for selected unbound/stale pin rows with id, display name,
  mux-name, optional socket, launch argv, and store-path fields;
  `Pins > remove` opens a confirmation modal naming the pin and
  exact source store path. Both Controls actions and row-level `R`
  / `Delete` accelerators now write through shared pin helpers
  instead of shelling out. Edit preflights duplicate id and
  duplicate mux conflicts before mutating the TOML store.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PIN-023`
