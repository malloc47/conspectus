---
id: CSP-027
title: Read Atelier fork index metadata
status: Done
assignee: []
created_date: '2026-05-15 03:58'
labels:
  - p2
milestone: m-3
dependencies:
  - CSP-026
ordinal: 27000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: parse `.atelier/forks/index.toml` into provider-neutral fork records
  with source metadata for worktree, selected, research, and standalone
  fork-like contexts.
- Tests: fixture tests for empty indexes, worktree-mode forks,
  selected-mode forks, research forks, standalone repo forks, and malformed
  fork entries.
- Manual checks: confirm parsing remains read-only and does not write
  `.conspectus.toml` or provider metadata.
- Blockers: `CSP-026`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added read-only `.atelier/forks/index.toml` parsing with
provider-neutral fork records for worktree, selected, research, standalone,
parent, repo membership, and harness lineage metadata; missing indexes load
as empty.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P2-007`
