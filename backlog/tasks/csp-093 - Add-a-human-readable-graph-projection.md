---
id: CSP-093
title: Add a human-readable graph projection
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-obs
milestone: m-11
dependencies: []
ordinal: 110000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `conspectus graph --format json` is the only graph output today.
  Add `--format text` (or a separate `conspectus graph --tree`) that
  renders nodes grouped by repo/workspace with linked sessions, mux, PR,
  and fork lineage. This is the workflow `atelier status` used to cover.
- Tests: snapshot tests for empty, sparse, and dense fixtures.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-OBS-001`
