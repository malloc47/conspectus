---
id: CSP-267
title: >-
  Descend into scan roots when looking for atelier workspaces (companion to
  `CSP-196`)
status: To Do
assignee: []
created_date: '2026-05-24 21:02'
labels:
  - t8
milestone: m-13
dependencies: []
ordinal: 435000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `src/discovery/atelier.rs::find_atelier_config`
  currently walks only **upward** from each scan root looking
  for `atelier.toml`, so a scan root one directory above a
  nested workspace (e.g. `--scan-root ~/src` with the workspace
  at `~/src/sysadmin/atelier.toml`) never produces a
  `Workspace` node and the sessions view's `graph` grouping
  cannot surface a workspace header. Extend the discovery so
  each scan root also scans its immediate children for an
  `atelier.toml` (and, optionally, two levels deep behind a
  config knob) before emitting an empty atelier fragment. Use
  a bounded depth so the cost stays predictable; skip
  well-known noise directories (`.git`, `node_modules`,
  `target`, `.cache`, `.atelier`). When multiple atelier
  configs are found, each yields its own `Workspace` node
  independently.
- Reproducer: with no `[tui].scan_roots` set and the operator
  launching from a directory outside their atelier workspaces
  (e.g. `cd ~/src/conspectus`), the auto-fallback scan root
  becomes `~/src/conspectus`. Atelier discovery's upward walk
  sees no `atelier.toml` even though `~/src/sysadmin/atelier.toml`
  exists in the same `~/src/` tree, so the sessions view shows
  no workspace headers. The `graph` and `repo` grouping modes
  visibly diverge only when at least one workspace is found,
  so this gap also makes the grouping options look redundant
  on cross-project setups.
- Tests: discovery-level tests covering (a) `atelier.toml` at
  the scan-root level (existing behavior, regression guard),
  (b) `atelier.toml` one directory down from the scan root
  (the fix), (c) multiple atelier configs nested under one
  scan root all surfacing as distinct `Workspace` nodes,
  (d) `atelier.toml` found via the upward walk still works
  (no regression on the `--scan-root ~/src/sysadmin/config`
  pattern), (e) the noise-directory exclusion list is
  honored.
- Blockers: none directly. Pairs naturally with `CSP-196`:
  once auto-broaden picks the right starting directory, this
  story makes the workspaces beneath it discoverable. Either
  can ship without the other; together they remove the
  "give me a multi-project view" friction.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-021`
