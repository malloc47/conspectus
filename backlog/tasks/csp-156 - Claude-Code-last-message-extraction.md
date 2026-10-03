---
id: CSP-156
title: Claude Code last-message extraction
status: Done
assignee: []
created_date: '2026-05-19 03:32'
labels:
  - h-preview
milestone: m-11
dependencies: []
ordinal: 187000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`read_session_last_message_preview` walks the trailing
`TAIL_SCAN_BYTES` (32 KiB) of each Claude Code JSONL transcript
backward, dropping the partial first line when the seek lands
mid-file, and returns the first user/assistant text content it
finds. Tool-use, tool-result, and thinking blocks are skipped;
so are `system` records and any user/assistant record with
`isCompactSummary: true`. The result goes through
`normalize_last_message_preview` so the cell is whitespace-
normalized and capped at 200 chars with a trailing `…`. The
extractor short-reads through `serde_json::from_slice` against
a minimal `MessageScan` / `MessageBody` / `MessageContent`
grammar (untagged enum covers both string content and the
modern content-block array). Discovery stays best-effort:
corrupt JSON / empty transcripts / tool-only tails yield
`None`. Eight unit tests pin the plain exchange, the
skip-tool-blocks path, thinking-skip, compaction-summary skip,
tool-only None, empty/corrupt None, the 200-char cap, and the
whitespace collapse. Live validation: running
`conspectus table sessions --columns id,agent,preview` against
`~/.claude/projects/` surfaces meaningful one-line previews
for every session that has any text in its tail window.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PREVIEW-002`
