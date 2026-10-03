---
id: CSP-006
title: Add baseline module boundaries
status: Done
assignee: []
created_date: '2026-05-15 02:31'
labels:
  - p0
milestone: m-1
dependencies:
  - CSP-004
ordinal: 6000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add minimal `model`, `resolve`, `discovery`, and `output` module
  boundaries aligned with ADR 0007, without committing a graph schema yet.
- Tests: module-level compile coverage through `cargo check`.
- Manual checks: inspect public module layout for ADR 0007 alignment.
- Blockers: `CSP-004`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P0-003`
