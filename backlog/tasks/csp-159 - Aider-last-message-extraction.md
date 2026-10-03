---
id: CSP-159
title: Aider last-message extraction
status: Done
assignee: []
created_date: '2026-05-19 03:32'
labels:
  - h-preview
milestone: m-11
dependencies:
  - CSP-155
ordinal: 190000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Blockers: `CSP-155`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Deferred per the story's escape clause. Two reasons:
(1) aider's `.aider.chat.history.md` is free-form markdown
with no formally-specified turn-delimiter, and the format
changes between aider versions, so a heuristic parser would
silently emit nonsense previews on any future-version
transcript; (2) `.aider.input.history` only carries user
inputs and would leave the preview misleading (no assistant
text). A `TODO(CSP-159)` comment in
`src/discovery/harness/aider.rs` pins the adapter on
`last_message_preview: None` and points at this entry.
Reopen this story when either (a) aider publishes a stable
structural marker for assistant turns or (b) a fixture
corpus is available to validate a heuristic parser against.
Aider sessions continue to discover with all other metadata;
the preview cell simply renders `—`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PREVIEW-005`
