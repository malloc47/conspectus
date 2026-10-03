---
id: CSP-177
title: Populate `AgentSessionNode.last_active_epoch` across harness adapters
status: Done
assignee: []
created_date: '2026-05-19 19:33'
labels:
  - h-agent
milestone: m-13
dependencies:
  - CSP-167
ordinal: 405000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend `AgentSessionNode` with an optional
  `last_active_epoch: Option<i64>` field (Unix seconds) and
  populate it from each supported harness adapter
  (`claude-code`, `codex`, `opencode`) using the freshest of
  transcript mtime, session-file mtime, or harness-recorded
  activity timestamp. The TUI sessions row-tree builder and the
  `conspectus table sessions` projection both consume this when
  present; without it, the row's recency column is blank and
  sessions sort alphabetically inside a group instead of
  recency-first.
- Tests: per-adapter unit tests for epoch extraction;
  snapshot test that the sessions row tree's `activity_epoch`
  / `recency` cells are populated for at least one harness
  fixture; an integration test that the resolver and JSON
  output round-trip the new field.
- Blockers: none; can land independently of further P8 stories,
  but CSP-167's discovery refresh path benefits when this lands
  before snapshot tests on the rendered v1 TUI freeze.
- Tests: adapter unit coverage for Claude, Codex, and opencode
  activity; row-tree test for `activity_epoch` / `recency`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`AgentSessionNode` now carries optional
`last_active_epoch`. Claude Code and Codex populate it from
transcript / rollout file mtime, opencode uses `time_updated`
/ `time_created` from sqlite or legacy `info.json`, and aider
uses the freshest history marker mtime. The TUI sessions row
tree now feeds this into the recency column, and
`conspectus table sessions --columns +activity` can display the
same relative age outside the TUI.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-AGENT-EPOCH`
