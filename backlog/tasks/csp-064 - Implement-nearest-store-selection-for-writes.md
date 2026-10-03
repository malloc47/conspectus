---
id: CSP-064
title: Implement nearest-store selection for writes
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-061
  - CSP-062
ordinal: 60000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a pure store-selection helper that decides where a new
  user-authored declaration belongs: project-local for relationships
  rooted in a discovered repo/workspace/checkout, global for orphan or
  user-wide relationships, and never in cache/index storage. Reuse the
  config walk rules from ADR 0012.
- Tests: unit tests for repo-rooted, workspace-rooted,
  checkout-rooted, branch/PR-rooted, mux-only, orphan-agent,
  multi-root, missing-root, and outside-home scenarios.
- Manual checks: inspect selected paths for representative repos,
  linked worktrees, and non-repo directories.
- Blockers: `CSP-061`, `CSP-062`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a pure `select_store_for_declaration` helper that
resolves declared-link writes to the nearest project config for
repo, workspace, checkout, session cwd, mux cwd, branch/PR, and
fork-rooted relationships, and falls back to the user config for
orphan relationships without touching cache or index storage.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-005`
