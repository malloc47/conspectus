---
id: CSP-681
title: Show ghost muxes in the Mux view
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-667
  - CSP-672
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: medium
type: feature
ordinal: 739000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Ghost muxes are the most common history entry, so they also show inline in the Mux view. A session that a ghost last hosted already has its own row in the Sessions view, so it gets a chip instead of a placeholder.

- Scope: ghosts sort below live muxes in each group, dimmed, with `gone` status and last-seen age; a Controls toggle hides them; the header reads `N mux · M gone`; `Enter` on a ghost rebuilds it; sessions last hosted by a ghost show a "was in <mux>" chip.
- Tests: row and snapshot tests with ghosts, the toggle, and the header.
- Manual checks: `conspectus tui --snapshot --snapshot-pane left` with ghosts present.
- Blockers: the graph projection and rebuild engine tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Ghost muxes appear dimmed below live ones and can be hidden from Controls
- [ ] #2 The header counts live and gone muxes separately
- [ ] #3 Sessions last hosted by a ghost carry a chip naming it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
