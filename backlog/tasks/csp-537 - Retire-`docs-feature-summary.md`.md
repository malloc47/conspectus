---
id: CSP-537
title: Retire `docs/feature-summary.md`
status: Done
assignee: []
created_date: '2026-09-30 18:30'
labels:
  - rel
milestone: m-20
dependencies: []
priority: high
ordinal: 604000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: the Phase 6 snapshot says there is "no MCP server, daemon, or
  caching layer yet" and omits the TUI, pins, worktrees, the transcript
  viewer, and the exports. `README.md` now carries the feature
  inventory. Per decision 5, delete the file and its `docs/index.md`
  entry (`:21`). Nothing else links to it except two historical mentions
  in this backlog.
- Tests: docs-only; link check over `README.md` and `docs/*.md`.
- Blockers: decision 5.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Deleted per decision 5, along with its `docs/index.md`
entry. `README.md` is the feature inventory.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `REL-006`
