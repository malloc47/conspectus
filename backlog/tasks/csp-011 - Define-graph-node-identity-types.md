---
id: CSP-011
title: Define graph node identity types
status: Done
assignee: []
created_date: '2026-05-15 02:36'
labels:
  - p1
milestone: m-2
dependencies:
  - CSP-010
ordinal: 11000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: implement structured node IDs from ADR 0001 for `Repo`,
  `Checkout`, `Workspace`, `AgentSession`, `MuxSession`, `Branch`, `Fork`,
  and `ForgePr`.
- Tests: unit tests for ID construction, display/debug behavior, serde round
  trips, and deterministic ordering.
- Manual checks: inspect JSON snippets from unit fixtures for stable ID
  shape.
- Blockers: `CSP-010`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P1-001`
