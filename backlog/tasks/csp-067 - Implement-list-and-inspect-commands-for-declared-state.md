---
id: CSP-067
title: Implement list and inspect commands for declared state
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-062
  - CSP-066
ordinal: 63000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add read-only commands that render declared links from local
  and global stores, including active, ignored, and overridden
  entries, their selected store, provenance, relation, endpoints, and
  reasons.
- Tests: CLI integration tests for empty stores, local declarations,
  global declarations, both stores, ignored/overridden entries,
  malformed config diagnostics, and deterministic output.
- Manual checks: create hand-written local/global declared entries
  and inspect list output.
- Blockers: `CSP-062`, `CSP-066`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Implemented read-only `declared list` output for user and
discovered project stores, including store, provenance, state, id,
relation, source/target endpoints, reason, override id, label, and
config path; output is deterministic, empty stores print nothing,
and malformed declared config emits a warning without mutating files.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-008`
