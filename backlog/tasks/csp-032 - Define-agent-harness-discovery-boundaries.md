---
id: CSP-032
title: Define agent harness discovery boundaries
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-031
ordinal: 32000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add read-only harness discovery traits, source-state inputs, and
  graph-fragment outputs for `AgentSession` nodes without binding the public
  graph model to provider-private schemas.
- Tests: unit tests for empty harness discovery, missing state directories,
  and deterministic fragment merging.
- Manual checks: inspect module boundaries for ADR 0007 alignment and confirm
  harness discovery does not perform output rendering.
- Blockers: `CSP-031`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a `discovery::harness` module with a `HarnessAdapter` trait
and `HarnessDiscovery` provider; extended `DiscoveryContext` with per-harness
state-root overrides; covered empty adapters, missing state roots, state-root
passthrough, and deterministic fragment merging.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-001`
