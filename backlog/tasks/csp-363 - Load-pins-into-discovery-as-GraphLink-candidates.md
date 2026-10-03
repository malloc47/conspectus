---
id: CSP-363
title: Load pins into discovery as GraphLink candidates
status: Done
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin
milestone: m-11
dependencies:
  - CSP-362
ordinal: 296000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a read-only `discovery::pins` pass analogous to
  `discovery::declared`. Map each entry into a new `Pin` candidate
  kind carrying `(id, harness, cwd, display_name, mux.backend,
  mux.name, mux.socket_name, store)`. `LocalPin` vs `GlobalPin`
  provenance follows config-file location (mirror ADR 0014 rule).
  Local beats global on `id` collision. Emit a diagnostic when both
  stores claim the same id with conflicting fields.
- Tests: unit tests for local-only, global-only, local-over-global,
  malformed-file diagnostic isolation, empty stores, config paths
  matching ADR 0012, and project configs discovered from observed
  session CWDs outside the startup scan roots.
- Blockers: `CSP-362`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`discovery::pins` loads local and global pins into
`PinCandidate` evidence, preserves sparse/malformed-store
behavior, reports duplicate/local-over-global diagnostics, and
aggregates project-local pin stores from scan roots plus observed
graph roots so pins stay stable across launch CWDs.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-003`
