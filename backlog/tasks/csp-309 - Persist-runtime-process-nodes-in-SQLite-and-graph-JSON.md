---
id: CSP-309
title: Persist runtime process nodes in SQLite and graph JSON
status: Done
assignee: []
created_date: '2026-05-31 19:22'
labels:
  - h-muxproc-fu
milestone: m-11
dependencies:
  - CSP-308
ordinal: 251000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend the query schema/loader/reader for runtime process
  nodes and their relation evidence. Keep process observations
  rebuildable and outside user-authored declared-link intent.
- Tests: schema constant tests, load/read parity snapshots, and graph
  JSON snapshots covering single-agent, multi-agent, subagent, stale
  argv, and unreadable-process cases.
- Blockers: `CSP-308`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Bumped the query schema version and added
`node_runtime_processes`, `v_nodes` coverage, loader insertion,
readback, `NodeId` JSON round-trip coverage, and full-snapshot
SQLite round-trip coverage. Minimal `node show` and TUI detail
summaries can render runtime process nodes once discovery emits
them. Scenario-specific process fixtures remain in
`CSP-312`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-FU-003`
