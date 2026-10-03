---
id: CSP-095
title: Add filter flags for the graph and session commands
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-obs
milestone: m-11
dependencies: []
ordinal: 112000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `--only-ambiguous`, `--only-unresolved`, `--only-orphan`, and a
  `--kind {agent_session|mux|repo|fork|pr}` filter. The graph today
  forces consumers to do their own filtering on JSON.
- Tests: CLI integration tests against existing snapshots.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-OBS-003`
