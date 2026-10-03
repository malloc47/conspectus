---
id: CSP-109
title: Settle declared-link conflict and override semantics
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-design
milestone: m-11
dependencies: []
ordinal: 142000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `docs/design.md` "Remaining Design Questions" asks (a) whether
  an override suppresses a single candidate, all candidates of a
  relation kind, or all links between two nodes; (b) whether "ignored"
  is node-level, link-level, or both; (c) the merge rule across local,
  global, discovered, and cached evidence; (d) whether confirmation
  creates a durable declared link even if the discovered evidence
  disappears. Record the conclusions in a new ADR and tighten the
  resolver tests.
- Tests: resolver tests for each conflict scenario.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-DESIGN-003`
