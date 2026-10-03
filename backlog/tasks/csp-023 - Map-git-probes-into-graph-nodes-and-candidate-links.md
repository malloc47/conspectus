---
id: CSP-023
title: Map git probes into graph nodes and candidate links
status: Done
assignee: []
created_date: '2026-05-15 03:58'
labels:
  - p2
milestone: m-3
dependencies:
  - CSP-022
ordinal: 23000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: emit `Repo`, `Checkout`, and `Branch` nodes plus links for repo
  membership and checked-out branch evidence from git probe results.
- Tests: JSON snapshot tests for a plain repo, a detached worktree, and a
  linked worktree fixture.
- Manual checks: run `cargo run -- graph --format json` from a plain git repo
  and inspect repo/checkout/branch identity shape.
- Blockers: `CSP-022`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Mapped git probe results into `Repo`, `Checkout`, and `Branch`
nodes with strong-discovered candidate links for repo membership and checked
out branches, plus fixed-path JSON snapshots for plain, detached, and linked
worktree cases.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P2-003`
