---
id: CSP-296
title: Thread subagent metadata into the graph model
status: To Do
assignee: []
created_date: '2026-05-28 23:02'
labels:
  - h-subagent
milestone: m-11
dependencies:
  - CSP-295
ordinal: 352000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add an optional boolean or enum field on `AgentSessionNode`
  (e.g. `session_kind: Option<SessionKind>` with variants `Human` /
  `Subagent`) so the classification survives into table rendering,
  resolver logic, and TUI row construction. Decide during the story
  whether to make this harness-agnostic or opencode-specific.
- Tests: model round-trip tests; sparse-serialization tests (absent
  field stays absent for non-opencode sessions).
- Blockers: `CSP-295`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SUBAGENT-002`
