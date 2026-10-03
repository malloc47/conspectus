---
id: CSP-017
title: Implement resolver precedence rules
status: Done
assignee: []
created_date: '2026-05-15 02:36'
labels:
  - p1
milestone: m-2
dependencies:
  - CSP-016
ordinal: 17000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: apply default precedence: local declared, global declared, strong
  discovered evidence, convention, then cached evidence.
- Tests: table-driven tests for declared-over-discovered precedence,
  discovered-over-cached precedence, ignored candidates, overridden
  candidates, and mux candidate precedence.
- Manual checks: inspect diagnostic output for conflicts and selected
  relationships.
- Blockers: `CSP-016`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P1-007`
