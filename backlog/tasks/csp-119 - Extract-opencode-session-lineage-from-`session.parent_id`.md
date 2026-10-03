---
id: CSP-119
title: Extract opencode session lineage from `session.parent_id`
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-lineage
milestone: m-11
dependencies: []
ordinal: 182000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Resolution: `src/discovery/harness/opencode.rs` now selects
  `parent_id` from the SQLite store and emits a `parent_session`
  candidate per row carrying a non-empty parent. Parent rows present
  in the same fragment resolve to concrete `AgentSession` endpoints;
  missing parents become `UnresolvedEndpoint` evidence with
  `harness_key = "opencode"` and the parent native id. Self-parent
  rows are skipped. Older schemas without `parent_id` fall back to a
  lineage-less SELECT rather than dropping every session.
  `lineage_kind` is `"unknown"` until opencode publishes operation
  semantics. Manual real-state validation pending.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-LINEAGE-003`
