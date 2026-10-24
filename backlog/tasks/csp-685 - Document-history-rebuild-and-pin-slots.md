---
id: CSP-685
title: 'Document history, rebuild, and pin slots'
status: Done
assignee: []
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 18:13'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-679
  - CSP-680
  - CSP-681
  - CSP-682
  - CSP-671
  - CSP-669
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/operations.md
  - docs/pins-walkthrough.md
priority: medium
type: docs
ordinal: 743000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operations guide, the pins walkthrough, the README, and the showcase and dev scenarios all describe the old pin model, and the new views need fixtures to be demonstrable and snapshot-testable.

- Scope: rewrite `docs/operations.md` Session pins and mux sections, `docs/pins-walkthrough.md`, and README mentions; add history and gone entities to the showcase scenario (ADR 0070) and dev scenarios; update screenshots if any show pins (ADR 0113).
- Tests: `git diff --check`; scenario and fixture tests.
- Manual checks: walk through the new pins walkthrough on a scratch setup.
- Blockers: the TUI, history, and migration tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Operator docs describe history, rebuild, pin slots, the Pinned and History views, migration, and reaping, and no longer describe pin launch or adopt
- [x] #2 The showcase scenario includes pinned, planned, and gone entities of several kinds
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Rewrote operations.md, the pins walkthrough, README, worktrees, comparison, library-api, and related docs around slots, history, rebuild, and migration; added pins, gone entities, and a planned slot to the showcase world and fixture with scenario and screenshot tests; regenerated the README screenshots including new Pinned and History images; and recorded the change in the changelog and amendment notes on the affected ADRs.
<!-- SECTION:FINAL_SUMMARY:END -->
