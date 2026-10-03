---
id: CSP-021
title: Define local discovery orchestration boundaries
status: Done
assignee: []
created_date: '2026-05-15 03:58'
labels:
  - p2
milestone: m-3
dependencies:
  - CSP-020
ordinal: 21000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add discovery traits and a local discovery coordinator that can
  collect provider graph fragments, merge them into a `GraphSnapshot`, and
  leave candidate-link resolution to the existing resolver.
- Tests: unit tests for merging empty and single-provider graph fragments
  without dropping nodes or candidate links.
- Manual checks: inspect module boundaries for ADR 0007 alignment and confirm
  discovery does not perform output rendering.
- Blockers: `CSP-020`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added provider, context, graph-fragment, and local coordinator
boundaries; discovery merges fragments into an unresolved graph snapshot and
leaves resolution/output to existing modules.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P2-001`
