---
id: CSP-015
title: Add fixture builders for sparse graph scenarios
status: Done
assignee: []
created_date: '2026-05-15 02:36'
labels:
  - p1
milestone: m-2
dependencies:
  - CSP-012
  - CSP-013
ordinal: 15000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add internal test helpers for orphan sessions, mux-only rows,
  repo-only rows, unresolved lineage evidence, conflicts, and mux
  candidates.
- Tests: fixture self-checks through JSON snapshot coverage.
- Manual checks: verify fixtures are internal test helpers, not public API.
- Blockers: `CSP-012`, `CSP-013`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P1-005`
