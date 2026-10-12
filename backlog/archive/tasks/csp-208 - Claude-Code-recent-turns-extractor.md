---
id: CSP-208
title: Claude Code recent-turns extractor
status: To Do
assignee: []
created_date: '2026-05-22 23:53'
updated_date: '2026-10-12 13:12'
labels:
  - h-transcript
milestone: m-11
dependencies: []
ordinal: 196000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend the existing tail-scan reader to return the
  last N user/assistant turns. Continue to skip tool-use,
  tool-result, thinking, and `system` records. Handle
  compaction (ADR 0019 context): when a `compact_boundary`
  row is in scope, the post-compaction summary should be
  distinguishable in the returned data (e.g. a turn-kind
  flag) so the TUI can render it differently. Expand the
  tail-scan window when N turns are not found in the current
  window, bounded by a hard cap.
- Tests: fixture tests for the plain exchange, the
  compaction-summary case, a tool-only tail, and the window
  expansion path.
- Blockers: `CSP-207`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-004`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-10-12: archived. ADR 0112 renders session previews through the native viewer (ADR 0052), which already parses Claude Code, Codex, and opencode transcripts, so this recent-turns path is superseded. `CSP-585.02` implements the session preview.
<!-- SECTION:NOTES:END -->
