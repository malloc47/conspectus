---
id: CSP-661
title: 'Update the design doc for history, rebuild, and pin slots'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-22 01:10'
updated_date: '2026-10-22 02:34'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-656
  - CSP-657
  - CSP-658
  - CSP-659
  - CSP-660
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: medium
type: docs
ordinal: 719000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`docs/design.md` is the planning reference and must reflect accepted ADRs. Its Session Pins and Mux Lifecycle sections describe the pin model this milestone replaces.

- Scope: rewrite Session Pins and Mux Lifecycle; add history to State And Persistence; update Decisions and the mutation envelope citation; mark the plan doc as carried by the new ADRs.
- Tests: `git diff --check`.
- Manual checks: none.
- Blockers: the five decision ADRs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `docs/design.md` describes history, presence, pin slots, rebuild, and the Pinned and History views, citing the new ADRs, and no longer describes the ADR 0057 binding model as current
- [x] #2 The plan doc's status line says the ADRs carry its decisions
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Replace Session Pins with History And Presence, Pin Slots, and Rebuild sections; rewrite Mux Lifecycle and the worktree new-stream text.
2. Add history and pins.toml to State And Persistence and Decisions; cite ADR 0143; describe the new views; extend the ERD.
3. Mark the plan doc as carried by the ADRs.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`docs/design.md` now describes history and presence, pin slots, rebuild, the Pinned and History views, and ghost muxes, citing ADRs 0140-0144, with the envelope cited as ADR 0143 and the new relations in the ERD; the ADR 0057 binding model is gone from it. The plan doc's status line says ADRs 0140-0144 carry its decisions. Verified with `git diff --check`.
<!-- SECTION:FINAL_SUMMARY:END -->
