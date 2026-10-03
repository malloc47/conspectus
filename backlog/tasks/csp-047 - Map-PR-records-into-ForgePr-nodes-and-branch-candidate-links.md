---
id: CSP-047
title: Map PR records into ForgePr nodes and branch candidate links
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-046
ordinal: 45000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: emit one `ForgePr` node per record (extending `ForgePrNode`
  with an optional `updated_epoch` and `is_draft` so the resolver can
  rank candidates by recency and draft state) and a `BranchHasForgePr`
  candidate link from the matching `Branch` node — matched by repo
  identity (host/owner/repo) and head ref. When the branch is not in
  the graph, emit an unresolved-endpoint candidate link so the
  evidence survives until later discovery resolves it.
- Tests: graph-fragment tests for PRs whose head ref matches a
  discovered branch, PRs whose head ref is unknown to the graph, draft
  vs non-draft PRs, closed vs merged vs open state, and stable node IDs
  across repeated runs.
- Manual checks: inspect JSON from a fixture-backed adapter run for
  readable provenance and identity shape.
- Blockers: `CSP-046`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Extended `ForgePrNode` with `updated_epoch` and
`is_draft` (skipped from JSON when false / absent for sparse
output). Added `RepoContext` and `fragment_for_repo` in
`discovery::forge::github`: per record, emits a `ForgePr` node
plus a `BranchHasForgePr` candidate link targeting the discovered
`Branch` node when the short head ref is in the supplied set, or
an unresolved branch endpoint carrying host/owner/repo +
head_ref metadata otherwise. Eight new tests cover matched
branch, unknown ref, draft propagation, open/closed/merged
state, stable IDs, updated-epoch propagation, and the
empty-records case.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-003`
