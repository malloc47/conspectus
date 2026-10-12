---
id: CSP-590
title: Discover recently merged and closed pull requests
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
ordinal: 639000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Why: `gh pr list` defaults to open PRs and a limit of 30, so merged and closed PRs never enter the graph, and busy repos can drop open PRs past the 30th.
- Scope: fetch all open PRs (higher limit) plus PRs of any state updated recently, and merge them.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Open PRs are fetched with a limit well above 30
- [x] #2 PRs merged or closed within a recent window enter the graph with their state
- [x] #3 A PR returned by both queries appears once
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Two queries per remote: open (limit 100) and any state updated in the last 30 days (limit 100); merge by number.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Each GitHub remote is asked for open PRs (limit 100) and PRs of any state updated in the last 30 days (limit 100), merged by number, so recently merged and closed PRs enter the graph. Verified with `just check` (dedupe test) and the live `gh` call log.
<!-- SECTION:FINAL_SUMMARY:END -->
