---
id: CSP-098
title: Surface activity/recency in the session tables
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-obs
milestone: m-11
dependencies:
  - CSP-090
ordinal: 116000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `AgentSessionNode` exposes some recency metadata (claude-code
  cwd discovery propagates timestamps; mux activity epochs flow through
  candidate metadata) but the table renders no recency column. Add a
  "last activity" column derived from session, mux, and PR signals,
  using a relative formatting helper (`2h`, `3d`).
- Tests: snapshot tests for representative fixtures with normalized
  timestamps.
- Blockers: `CSP-090` (typed source-metadata fields makes recency
  extraction safer).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-OBS-006`
