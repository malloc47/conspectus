---
id: CSP-311
title: >-
  Move mux-cardinality and attribution resolver logic onto runtime process
  evidence
status: Done
assignee: []
created_date: '2026-05-31 19:22'
labels:
  - h-muxproc-fu
milestone: m-11
dependencies:
  - CSP-310
ordinal: 253000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: teach resolver/cross-link inference to derive
  `AgentSession -> MuxSession` from explicit process observations and
  process/session candidates. Cardinality rules should count
  non-subagent runtime process roles instead of re-parsing opaque link
  metadata.
- Tests: resolver tests for zero/one/multiple non-subagent processes,
  subagent exclusion, current-session evidence beating stale launch
  argv, and unresolved process diagnostics.
- Blockers: `CSP-310`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`resolve_snapshot` now derives compatibility
`AgentSession -> MuxSession` candidates from
`mux_contains_process` plus concrete process/session evidence,
without fanning out ambiguous `process_candidates_session` links.
Resolver metadata records the source process links and counts
non-subagent runtime process roles so subagent observations do not
inflate mux cardinality.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-FU-005`
