---
id: CSP-014
title: Define graph snapshot JSON output
status: Done
assignee: []
created_date: '2026-05-15 02:36'
labels:
  - p1
milestone: m-2
dependencies:
  - CSP-012
  - CSP-013
ordinal: 14000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add the deterministic top-level graph document with `nodes`,
  `candidate_links`, `resolved_relationships`, and `diagnostics`.
- Tests: snapshot tests for empty graph JSON and sparse graph fixtures.
- Manual checks: confirm key ordering and separation between candidate links
  and resolved relationships.
- Blockers: `CSP-012`, `CSP-013`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P1-004`
