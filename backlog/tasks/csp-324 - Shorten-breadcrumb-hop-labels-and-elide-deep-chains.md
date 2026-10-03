---
id: CSP-324
title: Shorten breadcrumb hop labels and elide deep chains
status: Done
assignee: []
created_date: '2026-06-01 15:17'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-314
ordinal: 429000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: today each breadcrumb hop renders the focused node's
  full display label, which eats the breadcrumb line after two
  hops. Render hops as `kind:short_tag` (e.g. `mux:editor`,
  `proc:claude·82310`) and add an elision rule
  (`first … last-N`) when the rendered chain exceeds the
  breadcrumb zone width. Tiebreak ambiguous short forms within a
  chain by suffixing the last-4 of the id when two hops would
  otherwise collide.
- Tests: unit tests for the short-form formatter across each
  node kind; collision-tiebreak tests for two hops with the same
  short label; rendering tests at narrow widths confirming
  elision (`first … last-N`) without dropping the current hop;
  snapshot coverage for a 4+ hop chain.
- Blockers: `CSP-314`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-038`
