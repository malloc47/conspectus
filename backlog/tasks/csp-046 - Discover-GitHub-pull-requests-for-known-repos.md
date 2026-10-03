---
id: CSP-046
title: Discover GitHub pull requests for known repos
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-045
ordinal: 44000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: for each discovered repo, invoke
  `gh pr list --json number,state,url,headRefName,baseRefName,
  updatedAt,headRepositoryOwner,headRepository,isDraft` (or equivalent)
  and parse the JSON array into provider-neutral PR records carrying
  provider/host/owner/repo/number/state/url, the head ref name, draft
  flag, and an updated-at timestamp. Skip rows missing required fields
  rather than failing the run.
- Tests: fake-runner tests for zero PRs, one PR, multiple PRs,
  malformed rows, missing optional fields, draft vs non-draft, and
  unavailable `gh`.
- Manual checks: drive the adapter with a fixture-backed `gh` JSON
  blob and inspect record shape; do not exercise real `gh` in tests.
- Blockers: `CSP-045`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `discovery::forge::github` with a
`PullRequestRecord` / `PullRequestState` provider-neutral row
shape and a `GhPullRequestParser` for `gh pr list --json` output.
The parser is tolerant: empty, malformed, or row-level-invalid
input degrades to an empty list rather than failing. RFC 3339
`updatedAt` strings parse to a UTC epoch via a small embedded
civil-date converter so the resolver can rank by recency without
adding a chrono dependency. Ten new unit tests cover empty,
malformed, single, multi-row, draft, missing-optional,
unknown-state, and offset-vs-Z timestamp inputs.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-002`
