---
id: CSP-308
title: Add runtime process graph model and relation kinds
status: Done
assignee: []
created_date: '2026-05-31 19:22'
labels:
  - h-muxproc-fu
milestone: m-11
dependencies:
  - CSP-305
ordinal: 250000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a provider-neutral `RuntimeProcess` node with ephemeral
  observation identity and sparse attributes for PID, parent PID,
  root pane PID, command, cwd, harness key, process role, depth, and
  observed epoch. Add relation kinds for mux contains/observes process
  and process identifies/candidates/unresolved agent session evidence.
  Preserve `AgentSession -> MuxSession` as the resolver-selected
  user-facing relationship.
- Tests: serde round trips, deterministic identity/order tests, sparse
  node serialization, and relation-kind serialization.
- Blockers: `CSP-305`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `RuntimeProcessId`, `RuntimeProcessNode`,
`RuntimeProcessRole`, a `GraphNode::RuntimeProcess` variant, a
`NodeId::RuntimeProcess` variant, and process relation kinds for
mux/process containment plus process/session identification and
candidate evidence. Model tests cover stable display and relation
serialization.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-FU-002`
