---
id: CSP-297
title: Filter and nest subagent sessions in the TUI
status: To Do
assignee: []
created_date: '2026-05-28 23:02'
labels:
  - h-subagent
milestone: m-11
dependencies:
  - CSP-296
  - CSP-119
ordinal: 353000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: teach the session row tree to either nest subagent sessions
  as expandable children under their parent session row, or collapse
  them behind a toggle that defaults to hidden. The behavior must
  not change the existing sort/recency order of human sessions.
  Decide during the story whether nesting (per ADR 0018 lineage
  edges) or flat filtering (per checkpoint-style visibility toggle)
  is the right first pass.
- Tests: TUI row-tree tests for subagent nesting/collapsing under
  parent, orphan subagent (parent not discovered) behavior, and
  toggle persistence across views.
- Blockers: `CSP-296`, `CSP-119`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SUBAGENT-003`
