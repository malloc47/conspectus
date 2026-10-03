---
id: CSP-051
title: Implement the agent projection table renderer
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-050
ordinal: 49000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: render one row per `AgentSession` with harness, cwd, preferred
  mux, preferred PR, and ambiguity flags using the indicator format
  from `CSP-050`. Orphan sessions stay visible with empty mux/PR cells.
- Tests: snapshot tests for orphan sessions, sessions with a single
  mux match, sessions with multiple mux candidates, fork-linked
  sessions, and sessions whose branch has a forge PR.
- Manual checks: review snapshots for column alignment and readable
  ambiguity indicators.
- Blockers: `CSP-050`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Agent projection renders AGENT / CWD / MUX / MUX/CONF /
PR / PR/CONF columns. Mux cell shows the preferred mux session
label (or `—` for orphans). Ambiguity marker `*` appears when
the agent has multiple candidate mux links. PR cell shows the
first available BranchHasForgePr candidate. Unit tests cover
orphan, single-match, ambiguous mux, and branch-with-PR cases.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-007`
