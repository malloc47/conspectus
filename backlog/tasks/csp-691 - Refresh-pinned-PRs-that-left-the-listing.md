---
id: CSP-691
title: Refresh pinned PRs that left the listing
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-666
  - CSP-668
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0116-lazy-forge-cadence-and-hard-refresh.md
priority: low
type: feature
ordinal: 749000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A pinned PR that was merged or closed drops out of the open-PR listing, so its snapshot stops updating. Pinned PRs are few, so fetching them individually is affordable.

- Scope: for pinned PRs absent from the latest listing, a lazy `gh pr view` refresh on the forge cadence (ADR 0116) that updates the history snapshot.
- Tests: tests over a fake `gh` runner for refresh and failure.
- Manual checks: pin a PR, merge it, and confirm the Pinned view shows `merged`.
- Blockers: the PR snapshot and slot store tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A pinned PR missing from the listing is refreshed on the forge cadence, and its state updates
- [ ] #2 Refresh failures leave the last snapshot and raise a diagnostic
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
