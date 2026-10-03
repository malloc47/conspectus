---
id: CSP-328
title: >-
  Hide edge meta (`provenance · confidence · state`) from link rows by default
  with an opt-in toggle
status: Done
assignee: []
created_date: '2026-06-01 16:11'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-315
ordinal: 433000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: today every link row in the explorer carries a trailing
  `discovered · high · active` line that exposes the resolver's
  provenance / confidence / state triple plus the `alt of …` edge
  state label. That detail is essential for operators
  actively diagnosing a resolver decision but adds noise to the
  primary "navigate the graph" use case the explorer is built
  around. Hide the trailing meta line on both link-row variants
  (single-link composite trailing row and multi-link group child
  row) by default. Keep the `★` resolver-winner marker and the
  `⚠` group-level conflict aggregate — those are navigation
  signals, not edge-meta noise. Add a per-session toggle
  (suggested `M` for "meta" via the Controls overlay primary
  surface per ADR 0031, since the unbound keys list reserves `M`
  for future merge actions — pick another letter if needed) and
  a `[tui.detail].show_edge_meta = false` config knob so users
  who routinely need the meta line can flip the default.
- Direction (do not design now): a future "edge detail view"
  or focused edge inspector is the natural home for richer
  edge-resolver diagnostics (full provenance chain, every
  candidate side-by-side, the resolver's tiebreak rule, etc.).
  This ticket is the v1 hide-by-default cut and is intentionally
  scoped to a visibility toggle; the deeper inspector is its
  own follow-up once the use cases are clearer.
- Tests: renderer tests confirming the meta line is suppressed
  by default and surfaces after the toggle; config-default test
  for the new `show_edge_meta` knob; coverage that `★` and `⚠`
  remain visible in the default (compact) mode; reducer test
  for the toggle preserving cursor row identity.
- Blockers: `CSP-315` renderer; Controls overlay entry slot
  (ADR 0031).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-042`
