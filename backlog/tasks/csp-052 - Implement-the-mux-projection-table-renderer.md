---
id: CSP-052
title: Implement the mux projection table renderer
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-050
ordinal: 50000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: render one row per `MuxSession` with backend, cwd, attached
  agent sessions (zero, one, or many), and ambiguity flags. Mux
  sessions with no attached agent remain visible.
- Tests: snapshot tests for zero / one / many attached agents and
  unavailable-tmux scenarios (no mux rows).
- Manual checks: review the snapshot output for alignment.
- Blockers: `CSP-050`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Mux projection renders MUX / CWD / AGENTS columns;
AGENTS lists `session-label [indicator]` for every attached
session in stable order; mux sessions with no attached agent
still appear with an `—` cell.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-008`
