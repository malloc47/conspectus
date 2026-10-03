---
id: CSP-376
title: TUI row tree integration
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-364
  - CSP-163
ordinal: 309000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend `build_sessions_tree` and the mux row builder to
  render a row per pin. Unbound pins render with a dim glyph and
  secondary `(pin · <harness> · ~/...)` text. Bound pins render
  like the underlying agent-session row plus a small star glyph
  (final glyph chosen against `Theme`; see open question below).
  Mux view renders the pin-derived mux as an ordinary mux row when
  bound; unbound pins do not synthesize a mux row.
- Tests: row-tree builder unit tests for empty/bound/unbound/stale/
  ambiguous/multi-pin fixtures; insta snapshots over a 80×24 TUI
  render.
- Blockers: `CSP-364`; friendlier after `CSP-163` parts 2-5 land
  the per-view row builders.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Sessions row tree emits unbound/stale pins under a
synthetic Pins group, marks bound agent-session rows with
`pin_id`, renders pin rows/status hints in the TUI, and covers
unbound/stale/bound/mixed/end-to-end resolver cases with unit
tests.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-016`
