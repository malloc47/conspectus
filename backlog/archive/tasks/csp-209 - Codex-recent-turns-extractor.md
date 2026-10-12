---
id: CSP-209
title: Codex recent-turns extractor
status: To Do
assignee: []
created_date: '2026-05-22 23:53'
updated_date: '2026-10-12 13:12'
labels:
  - h-transcript
milestone: m-11
dependencies: []
ordinal: 197000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend `read_rollout_last_message_preview` style
  extraction to return the last N `response_item` /
  `payload.type == "message"` turns with `user` / `assistant`
  roles, skipping reasoning / function_call /
  function_call_output / event_msg records. Apply the same
  channel-marker filter (`<turn_aborted>`,
  `<proposed_plan>`) used by `CSP-173` per-turn.
- Tests: fixture tests including the channel-marker filter
  applied across multiple turns.
- Blockers: `CSP-207`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TRANSCRIPT-005`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
2026-10-12: archived. ADR 0112 renders session previews through the native viewer (ADR 0052), which already parses Claude Code, Codex, and opencode transcripts, so this recent-turns path is superseded. `CSP-585.02` implements the session preview.
<!-- SECTION:NOTES:END -->
