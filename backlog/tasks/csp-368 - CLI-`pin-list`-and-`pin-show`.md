---
id: CSP-368
title: CLI `pin list` and `pin show`
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-364
  - CSP-367
ordinal: 301000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: render pins from local + global stores with their
  provenance, binding state (`bound` / `unbound` / `stale` /
  `ambiguous`), store path, and bound agent-session id when bound.
  `--bound` / `--unbound` / `--stale` filters. `pin show <id>`
  prints the full entry plus binding diagnostic if any.
- Tests: CLI integration tests for empty stores, mixed local/global,
  each binding state via fixture graphs, deterministic ordering.
- Blockers: `CSP-364`, `CSP-367`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`pin list` and `pin show` render pin store provenance,
binding state, bound sessions, launch argv, and diagnostics.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-008`
