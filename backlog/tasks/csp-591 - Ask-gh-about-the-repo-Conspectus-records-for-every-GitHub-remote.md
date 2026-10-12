---
id: CSP-591
title: 'Ask gh about the repo Conspectus records, for every GitHub remote'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-12 23:35'
updated_date: '2026-10-12 23:54'
labels:
  - p4-fu
milestone: m-9
dependencies: []
priority: high
ordinal: 640000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Why: Conspectus records the owner/repo of the first GitHub remote but lets `gh` choose which repo to query, so in a fork with `origin` and `upstream` the PRs can be filed under the wrong repo. PRs opened from a fork usually live in `upstream`, which is never queried.
- Scope: pass `--repo HOST/OWNER/REPO` explicitly and query each distinct GitHub remote.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `gh pr list` names the repo it queries with `--repo`
- [x] #2 Each distinct GitHub remote of a repo is queried, and its PRs carry that remote's owner and repo
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Pass --repo HOST/OWNER/REPO; query each distinct GitHub remote; record PRs under the remote they came from.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`gh pr list` now names the repo it queries with `--repo`, and every distinct GitHub remote of a repo is queried, with PRs recorded under that remote. Verified with `just check` (explicit-repo and multi-remote tests) and the live call log.
<!-- SECTION:FINAL_SUMMARY:END -->
