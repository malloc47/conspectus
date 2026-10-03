---
id: CSP-358
title: Treat harness session keys as opaque strings in runtime attribution
status: Done
assignee: []
created_date: '2026-06-04 14:26'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-136
ordinal: 247000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: document and enforce the rule that `AgentSessionId.session_key`
  is an opaque harness-native string. UUID-shaped extraction remains a
  conservative generic fallback for arbitrary blobs, but any path,
  command, fd target, hook payload, or state record with known harness
  context should use that harness's session-key grammar. Initial
  grammars: Codex/Claude UUID-shaped transcript keys, opencode `ses_…`
  keys, plus command-token extraction when the harness binary is known.
- Tests: extractor unit tests proving opencode `ses_…` ids are extracted
  from process commands and fd/path evidence, while unrelated UUIDs
  outside known harness paths are ignored.
- Manual checks: inspect `graph --format json` for a live opencode mux
  session and confirm `process_identifies_session` evidence names the
  `ses_…` session key instead of falling back to same-cwd candidates.
- Blockers: `CSP-136`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Active-pane process command extraction now uses
harness-aware session-key parsing. OpenCode `ses_…` ids are
first-class session keys in process command and fd/path evidence,
while UUID-shaped extraction remains a generic fallback for Codex,
Claude Code, and unknown harness contexts.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-016` (the 2026-06-04 story; the ID was used twice)
