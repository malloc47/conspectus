---
id: CSP-295
title: Determine how to detect subagent sessions from opencode state
status: To Do
assignee: []
created_date: '2026-05-28 23:02'
labels:
  - h-subagent
milestone: m-11
dependencies: []
ordinal: 351000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: inspect opencode's `session` table schema for a dedicated
  `kind`/`type`/`is_subagent` column. If one exists, prefer it. If
  not, design a fallback heuristic based on `parent_id` presence plus
  title patterns (`(@explore subagent)`, `(@general subagent)`). If
  the schema is producer-maintained and stable, prefer the schema
  field; if not, document the heuristic's boundary conditions and
  decay story.
- Tests: fixture tests that confirm detection of known subagent
  shapes and non-detection of human `/new` forks with the same
  `parent_id` but no subagent title markers.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SUBAGENT-001`
