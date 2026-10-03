---
id: CSP-359
title: Sweep remaining UUID-only extractor call sites
status: Done
assignee: []
created_date: '2026-06-04 14:26'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-358
ordinal: 248000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: audit discovery, viewer bridge, resolver metadata, and output
  helpers for UUID-shaped session-key assumptions. Replace them with
  either typed `AgentSessionId` comparisons or harness-aware opaque
  string extractors. Keep UUID-only helpers private to generic fallback
  paths and rename them so call sites must choose between generic and
  harness-aware extraction deliberately.
- Tests: add regression coverage for opencode `ses_…`, Codex UUID, and
  Claude UUID session keys in the same fixtures. Include at least one
  negative test where an ordinary path token does not become a session
  key.
- Manual checks: run a mixed Codex/opencode/Claude tmux graph smoke and
  verify resolved links and right-pane IDs preserve full external session
  keys.
- Blockers: `CSP-358`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The process-link extractor now exposes explicit
harness-aware helpers and renamed the UUID-only helper to
`generic_uuid_like_session_keys`, making generic UUID matching
visible at call sites. Command evidence no longer treats every
ordinary argv token as a session key; it recognizes session-bearing
command forms such as `resume <id>` / `-s <id>` and harness-specific
tokens such as opencode `ses_…`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-017` (the 2026-06-04 story; the ID was used twice)
