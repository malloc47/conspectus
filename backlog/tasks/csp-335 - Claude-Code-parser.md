---
id: CSP-335
title: Claude Code parser
status: Done
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 210000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- 13 fixture tests cover supports() variant gating,
  file-not-found, plain user+assistant, text + tool_use
  fan-out, string tool_result, list-shaped tool_result,
  thinking, compaction summary, empty bodies dropped,
  metadata records skipped, malformed lines skipped,
  first-cwd-wins, tool_use without input. 1165 nextest
  green.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/viewer/parser/claude_code.rs` reads
`<state_root>/projects/*/<session_key>.jsonl` (std-only
`read_dir` glob, no `glob` dep) and emits a
`TranscriptDocument` in the normalized model. Translation
rules:
- One `TranscriptTurn` per content block, so the renderer
  can fold tool / thinking blocks independently of the
  surrounding prose.
- String `message.content` → one `TurnKind::Message` turn.
- `text` block → `Message`. `thinking` block → `Thinking`.
  `tool_use` block → `ToolUse` with body `"<name>: <json>"`.
  `tool_result` block → `ToolResult` with body flattened
  (string content or joined text blocks).
- Compaction summary records (`isCompactSummary=true`) →
  one `CompactionSummary` turn rather than a regular user
  message.
- Non-`user`/`assistant` record types
  (`custom-title`, `agent-name`, `system`, `attachment`,
  `file-history-snapshot`, `last-prompt`,
  `permission-mode`, `queue-operation`, `ai-title`) are
  skipped — they're metadata.
- Empty bodies dropped; first-record cwd wins;
  `timestamp` parsed RFC3339 → `DateTime<Utc>`.
- Malformed lines and blank lines skip silently
  (matches the H-PREVIEW posture).
- Missing file → `ParseError::NotFound`; the bridge will
  map that to `TranscriptDocument::unavailable`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEWER-NATIVE-003`
