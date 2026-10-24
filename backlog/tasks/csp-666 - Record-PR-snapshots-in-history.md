---
id: CSP-666
title: Record PR snapshots in history
status: Done
assignee:
  - '@claude'
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 12:46'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-662
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: medium
type: feature
ordinal: 724000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A PR never goes away, but it drops out of the open-PR listing once merged or closed. History keeps a display snapshot so pinned PRs and the History view can show it afterwards.

- Scope: records with provider, host, owner, repo, number, title, state, URL, head ref, and last-listed time, updated whenever a listing returns the PR; PRs are never gone.
- Tests: tests that a PR missing from the next listing keeps its snapshot and last state.
- Manual checks: none.
- Blockers: the history store ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every PR returned by a forge listing has a history record with the snapshot fields
- [x] #2 A PR absent from later listings keeps its record and last-known state
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Recorder observes PRs freshly returned by a listing; PRs are never gone.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Every PR a forge listing returns gets a history record (provider, host, owner, repo, number, title, state, draft, URL, head ref), updated whenever a later listing returns it; a PR missing from a later listing keeps its snapshot and last state, and PR nodes history itself projected are never re-recorded as listings. Verified by recorder and merge unit tests.
<!-- SECTION:FINAL_SUMMARY:END -->
