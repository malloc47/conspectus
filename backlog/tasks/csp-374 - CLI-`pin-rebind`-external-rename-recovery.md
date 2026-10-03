---
id: CSP-374
title: CLI `pin rebind` (external-rename recovery)
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-366
  - CSP-369
ordinal: 307000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update `pin.mux.name` (and optionally `pin.mux.socket_name`)
  in the pin's owning TOML store. Validates that no other pin
  already targets the new mux triple. Does not touch tmux.
- Tests: integration tests for rebind on a stale pin, rebind into
  a duplicate (rejected), rebind across stores (refused — operator
  moves the entry instead).
- Blockers: `CSP-366`, `CSP-369`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`pin rebind` updates the owning pin store's mux target,
rejects duplicate mux triples, and leaves tmux state untouched.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-014`
