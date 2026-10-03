---
id: CSP-016
title: Implement the resolver skeleton
status: Done
assignee: []
created_date: '2026-05-15 02:36'
labels:
  - p1
milestone: m-2
dependencies:
  - CSP-013
  - CSP-015
ordinal: 16000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: accept GraphLink candidates and emit typed resolved relationships
  without deleting or mutating lower-priority evidence.
- Tests: table-driven resolver tests for sparse links, no-op empty graphs,
  unresolved lineage evidence, and conflict preservation.
- Manual checks: inspect resolver output for a sparse fixture and confirm
  candidate evidence remains present.
- Blockers: `CSP-013`, `CSP-015`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P1-006`
