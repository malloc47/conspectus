---
id: CSP-050
title: Define table output projection boundaries
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-042
ordinal: 48000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend `src/output/` with a `Projection` enum
  (`Agent`/`Mux`/`Union`) and a render trait that takes a resolved
  `GraphSnapshot` plus a projection and returns a deterministic
  plain-text table. Define the compact provenance / confidence /
  ambiguity indicator format up front (e.g. `LD/SD/D/C/$`,
  `H/M/L`, and an `*` marker for ambiguous selections) so all three
  renderers share it.
- Tests: unit tests for the indicator formatter and empty-graph
  rendering for each projection.
- Manual checks: inspect indicator output for representative candidate
  links.
- Blockers: `CSP-042`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `output::table` with `Projection` (re-exported
from `config`), a single `render` entry point, and an
`indicator(provenance, confidence, ambiguous)` helper that emits
cells like `LD/H` / `SD/M*` / `$/L`. Codes are LD / GD / SD /
D / C / $ for provenance and H / M / L for confidence.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-006`
