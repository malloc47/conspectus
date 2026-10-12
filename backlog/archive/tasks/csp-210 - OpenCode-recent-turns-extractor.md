---
id: CSP-210
title: OpenCode recent-turns extractor
status: To Do
assignee: []
created_date: '2026-05-22 23:53'
updated_date: '2026-10-12 13:12'
labels:
  - h-transcript
milestone: m-11
dependencies: []
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-10-12: archived. ADR 0112 renders session previews through the native viewer (ADR 0052), which already parses Claude Code, Codex, and opencode transcripts, so this recent-turns path is superseded. `CSP-585.02` implements the session preview.
<!-- SECTION:NOTES:END -->
