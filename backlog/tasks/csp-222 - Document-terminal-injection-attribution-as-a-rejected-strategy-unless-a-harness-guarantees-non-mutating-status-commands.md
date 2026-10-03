---
id: CSP-222
title: >-
  Document terminal-injection attribution as a rejected strategy unless a
  harness guarantees non-mutating status commands
status: Done
assignee: []
created_date: '2026-05-22 23:53'
labels:
  - h-muxproc
milestone: m-11
dependencies:
  - CSP-135
ordinal: 259000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: record the policy that Conspectus must not use
  `tmux send-keys`, slash commands, prompts such as `/status`, or
  terminal scraping to ask an agent for its current session id
  because these are user inputs and may mutate JSONL/transcript
  logs. The only exception is a harness-documented command channel
  that explicitly guarantees no transcript/session mutation; such
  an exception must be captured by `CSP-219` and implemented
  as a control-plane adapter rather than generic terminal input.
  Put the rationale in the process-linking ADR or a short follow-up
  ADR so future work does not rediscover the same tempting but
  unsafe approach.
- Tests: none.
- Related: `CSP-227` records why scraping or injecting
  Claude Code `/usage` is tempting but should not be treated as
  the preferred architecture unless no non-mutating control or hook
  source exists.
- Blockers: `CSP-135` ADR can absorb this if it has not
  landed; otherwise write a follow-up ADR.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0028 rejects terminal injection, slash-command
probing, and generic terminal scraping for current-session
attribution. Harness-documented non-mutating command channels
remain possible only as explicit control-plane adapters.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-008`
