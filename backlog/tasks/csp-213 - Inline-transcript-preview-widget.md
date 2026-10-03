---
id: CSP-213
title: Inline transcript-preview widget
status: To Do
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-transcript
milestone: m-11
dependencies:
  - CSP-207
  - CSP-212
ordinal: 201000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: new `src/tui/transcript_preview.rs` (or similar)
  that takes a `Vec<TranscriptTurn>` and produces a styled
  `ratatui::text::Text` filling the available right-panel
  height. Render per-turn role headers
  (e.g. dimmed `you`/`assistant` labels), use
  `tui-markdown` for the body, mark compaction-summary
  turns visibly, and crop or scroll when content exceeds
  the pane. Honor `--no-live-preview` by falling back to
  the existing single-line `last_message_preview`. Surface
  "transcript unavailable" and stale-data markers per ADR
  0023's privacy posture.
- Tests: Ratatui buffer snapshot tests for: (a) a normal
  multi-turn preview, (b) a Claude Code compaction-summary
  turn, (c) an "unavailable" case, (d) `--no-live-preview`
  falling back to the single-line preview.
- Blockers: `CSP-207`, `CSP-212`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-009`
