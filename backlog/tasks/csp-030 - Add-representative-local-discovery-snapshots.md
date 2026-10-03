---
id: CSP-030
title: Add representative local-discovery snapshots
status: Done
assignee: []
created_date: '2026-05-15 03:58'
labels:
  - p2
milestone: m-3
dependencies:
  - CSP-023
  - CSP-025
  - CSP-028
  - CSP-029
ordinal: 30000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: snapshot graph JSON for plain repo, linked worktree, generic
  workspace, Atelier workspace without forks, and Atelier workspace with
  worktree, selected, research, and standalone fork contexts.
- Tests: `cargo test --all-targets --all-features`; `cargo nextest run
  --all-targets --all-features`.
- Manual checks: review snapshots for stable ordering, readable provenance,
  and separation of candidate links from resolved relationships.
- Blockers: `CSP-023`, `CSP-025`, `CSP-028`, `CSP-029`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added normalized temp-fixture snapshots for plain repo, linked
worktree, generic workspace, Atelier workspace without forks, and Atelier
workspace with worktree, selected, and research fork metadata.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P2-010`
