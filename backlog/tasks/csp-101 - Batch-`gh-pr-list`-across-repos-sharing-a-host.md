---
id: CSP-101
title: Batch `gh pr list` across repos sharing a host
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-prod
milestone: m-11
dependencies:
  - CSP-100
ordinal: 134000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `GitHubForgeProvider` runs `gh pr list --json` once per
  discovered repo (`src/discovery/forge/mod.rs:67`). For workspaces
  with several repos on the same host/owner this multiplies the spawn
  cost. Investigate whether `gh search prs --owner` (or a parallelized
  batch invocation) is appropriate, and keep the per-repo path as a
  fallback.
- Tests: parser tests for the batched JSON; integration test with a
  `FakeGh` that records spawn counts.
- Blockers: `CSP-100` (caching narrows the urgency).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PROD-003`
