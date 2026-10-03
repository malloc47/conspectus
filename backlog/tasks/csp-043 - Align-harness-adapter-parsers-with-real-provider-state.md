---
id: CSP-043
title: Align harness adapter parsers with real provider state
status: Done
assignee: []
created_date: '2026-05-15 21:27'
labels:
  - p3-fu
milestone: m-10
dependencies: []
ordinal: 81000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend the Codex, Claude Code, and opencode adapters so the cwd
  and any activity/recency timestamps from real local state populate
  `AgentSessionNode.cwd` (and link metadata where applicable). The Phase 3
  `cross_link::infer` pass already correlates sessions and mux sessions on
  matching cwds, but real harness JSONL/info.json layouts left
  `agent_session.cwd` empty during the Phase 3 smoke test.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Codex now walks `sessions/` recursively (real Codex stores
rollouts under `sessions/YYYY/MM/DD/`), and the Claude Code adapter
scans up to 200 JSONL lines looking for the first event that carries
`cwd` (real sessions begin with a `permission-mode` envelope that lacks
`cwd`), falling back to a best-effort decode of the encoded project
directory name. Session ids are now taken from the file stem rather
than insisting on a first-line `sessionId`. A fresh smoke run from
inside the conspectus repo went from 16 cwd-less sessions to 34
sessions (17 codex + 17 claude-code) all carrying cwd, 12
`linked_to_mux` candidates, and 13 resolved relationships including
the live claude-code session attached to the `conspectus-smoke` tmux
session.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-FU-001`
