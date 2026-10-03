---
id: CSP-102
title: Add a graph-diff command
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-prod
milestone: m-11
dependencies: []
ordinal: 135000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `conspectus graph diff <a.json> <b.json>` (or save snapshots
  under `$XDG_DATA_HOME` and diff against the previous run). Useful for
  explaining "what changed since the last fork" and for Atelier
  delegation acceptance criteria.
- Tests: snapshot tests for added/removed nodes and links and changed
  resolution.
- Blockers: `CSP-100` if diffs reuse the cache layer.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PROD-004`
