---
id: CSP-053
title: Implement the union projection table renderer
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-051
  - CSP-052
ordinal: 51000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: render a single table that preserves both agent and mux rows
  plus their relationship status, with stable ordering so identical
  snapshots reproduce byte-for-byte. Use one row per node with a
  relationship column describing the preferred link and ambiguity.
- Tests: snapshot tests for empty graphs, sessions without a mux, mux
  without sessions, and one-to-many mux candidates.
- Manual checks: confirm the union table makes ambiguity visible
  without duplicating rows.
- Blockers: `CSP-051`, `CSP-052`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Union projection emits one row per node prefixed by
`agent` / `mux`. Agent rows carry a relationship column
formatted as `mux=<target> [indicator]` (`mux=—` when no
candidate exists). Mux rows have a `—` relationship cell since
attached sessions appear as their own agent rows. Unit tests
cover both kinds plus the empty-graph header-only case.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-009`
