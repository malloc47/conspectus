---
id: CSP-689
title: 'Release history, rebuild, and pin slots as 0.2.0'
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-661
  - CSP-669
  - CSP-670
  - CSP-671
  - CSP-673
  - CSP-676
  - CSP-679
  - CSP-680
  - CSP-681
  - CSP-682
  - CSP-683
  - CSP-684
  - CSP-685
  - CSP-541
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - CHANGELOG.md
priority: medium
type: chore
ordinal: 747000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The change retires most of the `pin` CLI and the `[[pins.entries]]` schema. Given its size, the operator wants it shipped with its own version bump after 0.1.0, so the break is visible in the version and the changelog.

- Scope: bump the crate version to 0.2.0; a CHANGELOG entry covering history, rebuild, pin slots, the new views, the breaking `pin` CLI changes, and `pin migrate`; tag and release following the process from CSP-541.
- Tests: `just check`.
- Manual checks: install the release over a 0.1.0 setup with legacy pins and run `pin migrate`.
- Blockers: the implementation tickets and CSP-541.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The crate version is 0.2.0 and the CHANGELOG describes the change, its breaking CLI changes, and migration
- [ ] #2 Upgrading from 0.1.0 keeps legacy pins visible until `pin migrate` runs
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
