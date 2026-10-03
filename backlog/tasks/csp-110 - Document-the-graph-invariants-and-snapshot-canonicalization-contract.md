---
id: CSP-110
title: Document the graph invariants and snapshot canonicalization contract
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-design
milestone: m-11
dependencies: []
ordinal: 143000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: callers (and the api facade) need to know when a
  `GraphSnapshot` is canonical, when cross-link inference has run, and
  what invariants hold after `discover_local_with` vs after
  `resolve_snapshot`. Add a short contract section to
  `docs/library-api.md` and consider asserting invariants in
  `merge_fragments`.
- Tests: unit tests for the documented invariants.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-DESIGN-004`
