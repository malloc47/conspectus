---
id: CSP-012
title: Define typed node models
status: Done
assignee: []
created_date: '2026-05-15 02:36'
labels:
  - p1
milestone: m-2
dependencies:
  - CSP-011
ordinal: 12000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add typed node structs/enums for the Phase 1 graph without
  provider-specific discovery behavior.
- Tests: unit tests for serde round trips and sparse node serialization.
- Manual checks: inspect representative serialized orphan session,
  mux-only, and repo-only nodes.
- Blockers: `CSP-011`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P1-002`
