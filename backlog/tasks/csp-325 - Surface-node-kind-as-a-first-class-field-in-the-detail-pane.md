---
id: CSP-325
title: Surface node kind as a first-class field in the detail pane
status: Done
assignee: []
created_date: '2026-06-01 15:17'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-313
  - CSP-324
ordinal: 430000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: today the node kind is buried in the harness-prefixed
  id (e.g. `opencode:ses_…`) and the operator has to parse it
  out. Render the kind as a dim leading chip (e.g.
  `[agent_session]`) in the Node zone title and in link / group
  rows, separately from the id/label. For the `cwd` core-field
  row specifically, perform a reverse lookup against the
  `GraphDb` and append the resolved-owning-node kind chip
  (`Repo`, `Workspace`, `Checkout`) when the lookup succeeds; on
  no match leave the path bare rather than guessing. Apply the
  same convention to the breadcrumb hop short-form from
  `CSP-324`.
- Tests: renderer tests for kind chips on each node kind across
  the Node zone, link rows, and group headers; reverse-lookup
  tests for `cwd` resolving to Repo / Workspace / Checkout / no
  match; snapshot coverage for a sparse-graph case where the
  `cwd` doesn't resolve.
- Blockers: `CSP-313`, `CSP-324` (so the breadcrumb short-form can
  pick up the chip too).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-039`
