---
id: CSP-018
title: Add `conspectus graph --format json`
status: Done
assignee: []
created_date: '2026-05-15 02:36'
labels:
  - p1
milestone: m-2
dependencies:
  - CSP-014
  - CSP-016
ordinal: 18000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add the CLI command that emits the Phase 1 graph document; the
  command may produce an empty graph or fixture-backed graph, but not local
  discovery.
- Tests: CLI integration tests for `graph --format json`, invalid formats,
  and deterministic output.
- Manual checks: `cargo run -- graph --format json`.
- Blockers: `CSP-014`, `CSP-016`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P1-008`
