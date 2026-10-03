---
id: CSP-157
title: Codex last-message extraction
status: Done
assignee: []
created_date: '2026-05-19 03:32'
labels:
  - h-preview
milestone: m-11
dependencies: []
ordinal: 188000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`read_rollout_last_message_preview` does the same
32 KiB tail-walk for codex rollouts and feeds the result
through `normalize_last_message_preview`. The codex grammar
differs from claude-code (records are
`{type: "response_item", payload: {type: "message", role,
content: [{type: "input_text"|"output_text", text}]}}` with
`reasoning` / `function_call` / `function_call_output` /
`event_msg` records to skip), so a minimal `RolloutLine` /
`RolloutPayload` / `RolloutContent` grammar handles
extraction. Only `response_item` records with
`payload.type == "message"` and a `user`/`assistant` role
contribute; the extractor returns the last non-empty
`input_text`/`output_text` block. Codex's rollout is keyed by
session id from the `session_meta` line, so previews are
stored in a `HashMap<String, String>` keyed by id and
attached to the matching node at construction. Six unit tests
cover the plain output_text case, skipping tool/reasoning/event
records, skipping empty text blocks, meta-only None,
200-char cap, and corrupt-tail None. Live validation: real
`~/.codex/sessions/**` rollouts produce meaningful previews
in `conspectus table sessions`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PREVIEW-003`
