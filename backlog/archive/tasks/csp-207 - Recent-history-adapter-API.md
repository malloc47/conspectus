---
id: CSP-207
title: Recent-history adapter API
status: To Do
assignee: []
created_date: '2026-05-22 23:53'
updated_date: '2026-10-12 13:12'
labels:
  - h-transcript
milestone: m-11
dependencies: []
ordinal: 195000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: define an on-demand adapter entry point on each
  harness that returns the last N user/assistant turns for a
  given `AgentSessionId` (parallel to the H-PREVIEW
  extractors but returning a structured `Vec<TranscriptTurn>`
  rather than a single normalized string). Decide whether the
  structure lives in `model::` (and is also usable by a
  future full-transcript viewer) or stays inside the harness
  module. Cap turns by count, by total bytes, or both;
  document the choice. Errors must degrade to "unavailable"
  without panicking the TUI.
- Tests: trait/contract tests; one fake harness adapter.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-003`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-10-12: archived. ADR 0112 renders session previews through the native viewer (ADR 0052), which already parses Claude Code, Codex, and opencode transcripts, so this recent-turns path is superseded. `CSP-585.02` implements the session preview.
<!-- SECTION:NOTES:END -->
