---
id: CSP-225
title: Implement hook-sidecar discovery provider
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-224
ordinal: 262000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: read the sidecar records defined by `CSP-224` and
  convert them into `LinkedToMux` candidate links. Match hook
  records to mux sessions by tmux pane id when present, then pane
  pid, then tmux session metadata, and only then cwd as a weak
  fallback. Match records to agent sessions by explicit session key
  or transcript/session path. Handle stale records conservatively:
  they may explain exited sessions in future views, but should not
  override fresh active-pane fd/process evidence.
- Tests: fixture directory with multiple harness records, stale
  records, malformed JSON, same-session duplicate events, tmux pane
  reuse, missing mux session, and missing agent session. Resolver
  tests for ranking relative to `active_pane_fd_session_match`.
- Manual checks: run with a live hook-enabled session and confirm
  `graph --format json` shows the hook evidence without requiring
  transcript scraping or terminal input.
- Blockers: `CSP-224`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`discovery::hook_sidecar` reads fresh JSON records after
harness and tmux discovery, emits high-confidence `LinkedToMux`
candidates, and marks stale `active_pane_command_session_match`
candidates for the same mux as overridden. It also synthesizes a
sparse `AgentSession` node from fresh hook records when Claude Code
has fired `SessionStart` but has not yet persisted the transcript
file because the new session has no messages.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-011`
