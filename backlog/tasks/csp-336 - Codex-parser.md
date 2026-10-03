---
id: CSP-336
title: Codex parser
status: Done
assignee: []
created_date: '2026-06-02 18:59'
labels:
  - h-viewer-native
milestone: m-11
dependencies: []
ordinal: 211000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- 16 fixture tests cover supports() gating, file-not-found,
  session_meta cwd capture, user/assistant message turns,
  developer/system skip, function_call → ToolUse,
  function_call_output → ToolResult, reasoning with summary
  → Thinking, encrypted-only reasoning skip, event_msg skip,
  `<turn_aborted>` and `<proposed_plan>` drops, compacted →
  CompactionSummary, web_search_call → ToolUse, malformed
  skip, nested-path file lookup. 1181 nextest green.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/viewer/parser/codex.rs` walks
`<state_root>/sessions/<year>/<month>/<day>/` with std
`read_dir` (no `walkdir` dep) and locates the rollout file
by `-<session_key>.jsonl` suffix match. Translation rules
capture the dual-family record schema:
- Outer `type` = `session_meta` → fill `TranscriptMeta.cwd`
  (first wins), no turn.
- Outer `type` = `turn_context` → cwd fallback only, no turn.
- Outer `type` = `response_item` → dispatch on
  `payload.type`:
  - `message` with role `user`/`assistant` → one `Message`
    turn per `input_text`/`output_text` content block;
    `developer` and `system` roles skipped as injected
    instructions.
  - `reasoning` → `Thinking` (joined `summary` + `content`
    text blocks); skipped if only `encrypted_content`.
  - `function_call`, `custom_tool_call`, `web_search_call`
    → `ToolUse` with body `"<name>: <arguments>"`.
  - `function_call_output`, `custom_tool_call_output` →
    `ToolResult` with body = output.
- Outer `type` = `compacted` → `CompactionSummary` turn from
  `payload.message`.
- Outer `type` = `event_msg` → skipped (engine telemetry:
  token_count, task_started, exec_command_end echoes, etc.).
- Channel-marker filter per CSP-173: message bodies
  whose trimmed content is exactly `<turn_aborted>` or
  `<proposed_plan>` are dropped as not-real-user-text.
- RFC3339 timestamps → `DateTime<Utc>`; malformed and blank
  lines skip silently.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEWER-NATIVE-004`
