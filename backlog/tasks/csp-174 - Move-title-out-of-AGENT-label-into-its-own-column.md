---
id: CSP-174
title: Move title out of AGENT label into its own column
status: Done
assignee: []
created_date: '2026-05-19 12:06'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 192000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`agent_session_label` no longer consults
`AgentSessionNode.title`. Every row renders
`harness:<session_key>`, with `agent_session_key_for_label`
deferring to `short_session_id` only when the key exceeds 32
chars. UUIDs (claude-code, codex) collapse to `…<last-8>`;
shorter human-readable keys
(`session-alpha`, opencode `ses_…`) pass through verbatim, so
every existing insta snapshot stayed byte-stable. A new
`title` column is registered on the `sessions` and `union`
row-types as opt-in; the extractor reads
`AgentSessionNode.title` directly. Union row-type renders `—`
for mux rows. Live verification on this workspace: opencode
rows that used to render
`opencode:tmux clipboard not syncing over SSH (fork #1)` in
AGENT now show `opencode:ses_204a14312…` with the chat topic
moving to the new `TITLE` column. Four new unit tests cover
the AGENT-cell label without title, UUID truncation, short-
key verbatim, and the new `title` column for both sessions and
union row-types. 449 tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-015`
