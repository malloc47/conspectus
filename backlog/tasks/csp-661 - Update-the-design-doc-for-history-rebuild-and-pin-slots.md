---
id: CSP-661
title: 'Update the design doc for history, rebuild, and pin slots'
status: To Do
assignee: []
created_date: '2026-10-22 01:10'
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
- [ ] #1 `docs/design.md` describes history, presence, pin slots, rebuild, and the Pinned and History views, citing the new ADRs, and no longer describes the ADR 0057 binding model as current
- [ ] #2 The plan doc's status line says the ADRs carry its decisions
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
