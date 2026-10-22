---
id: CSP-666
title: Record PR snapshots in history
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
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
- [ ] #1 Every PR returned by a forge listing has a history record with the snapshot fields
- [ ] #2 A PR absent from later listings keeps its record and last-known state
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
