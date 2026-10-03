---
id: CSP-210
title: OpenCode recent-turns extractor
status: To Do
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-transcript
milestone: m-11
dependencies:
  - CSP-207
ordinal: 198000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend the modern `part` table reader to return the
  most recent N `type: "text"` rows per session via a single
  SQL query (analogous to `read_last_message_previews`).
  Degrade to an empty result when the `part` table is absent
  so legacy stores still surface session metadata.
- Tests: SQLite-backed fixture tests covering most-recent
  ordering, per-session attribution, and the absent-table
  fallback.
- Blockers: `CSP-207`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-006`
