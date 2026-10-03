---
id: CSP-337
title: OpenCode parser (SQLite-of-record)
status: Done
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 212000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- 12 SQLite-backed fixture tests cover supports() gating,
  missing-db, empty-session-with-cwd, user/assistant
  role-from-message, reasoning → Thinking, tool → ToolUse +
  ToolResult split, tool-without-output → ToolUse only,
  step/patch skip, intra-message time_created ordering,
  other-session exclusion, empty-body drop, epoch-ms round
  trip.
- Real-data smoke (one-off, not committed): parsed a 381-msg /
  1658-part session from the author's `opencode.db` into 1193
  turns with kind histogram Message: 65, Thinking: 349,
  ToolResult: 388, ToolUse: 391 — confirms the parser
  matches the live schema.
- 1193 nextest green. **Closes the OpenCode coverage gap**
  that `CSP-330` could not.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/viewer/parser/opencode.rs` reads `opencode.db`
via `rusqlite` (`OpenFlags::SQLITE_OPEN_READ_ONLY |
SQLITE_OPEN_NO_MUTEX`). One LEFT JOIN between `message` and
`part` keyed on `session_id` returns the conversation in
`(m.time_created, p.time_created)` order. `session_diff/`
is never touched — the SQLite store is authoritative, which
is the gap that recall could not close. Translation rules:
- `session.directory` → `TranscriptMeta.cwd`.
- `part.data.type = "text"` → `Message` turn with role
  taken from the parent `message.data.role` (`user` or
  `assistant`).
- `part.data.type = "reasoning"` → `Thinking` turn.
- `part.data.type = "tool"` → emits **two** turns from the
  same row: a `ToolUse` with body `"<tool>: <state.input>"`
  and a companion `ToolResult` with body `state.output`.
  OpenCode bundles call+result in one row; the split keeps
  the renderer's fold-tool-blocks UX consistent with
  Claude / Codex.
- `step-start`, `step-finish`, `patch` parts skipped (model
  lifecycle markers; v1 doesn't render patches).
- `part.time_created` (epoch ms) → `DateTime<Utc>` via
  `chrono::DateTime::from_timestamp_millis`. Empty bodies
  dropped. Missing DB → `ParseError::NotFound`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEWER-NATIVE-005`
