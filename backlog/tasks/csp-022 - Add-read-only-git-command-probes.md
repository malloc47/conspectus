---
id: CSP-022
title: Add read-only git command probes
status: Done
assignee: []
created_date: '2026-05-15 03:58'
labels:
  - p2
milestone: m-3
dependencies:
  - CSP-021
ordinal: 22000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: shell out to `git` for repo common dir, worktree root, current
  branch/refname, remotes, upstream, and per-worktree metadata when available.
- Tests: integration tests using temporary git repos, detached HEADs, branch
  upstreams, and linked worktrees.
- Manual checks: run probes from a plain repo and linked worktree and verify
  no files are modified.
- Blockers: `CSP-021`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added read-only git probes for common dir, worktree root, git dir,
branch ref, upstream, and remotes with temp-repo coverage for plain,
detached, upstream, and linked-worktree cases.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P2-002`
