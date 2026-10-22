---
id: CSP-679
title: Add the Pinned view
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-678
  - CSP-668
  - CSP-672
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: medium
type: feature
ordinal: 737000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pinned entities get their own view, a peer of Sessions, Mux, PRs, and the others, replacing the Pins groups at the top of the Sessions and Mux views.

- Scope: a Pinned view in the view switcher; grouping project (default), kind, none; sort by slot order (default) or recency; mux slots expand to sessions and checkout slots to muxes and sessions; `Enter` runs the kind's primary action (attach, rebuild, resume, open mux in worktree, open PR); a row menu with edit, rename, retarget, refresh snapshot, unpin, reveal, and show plan; the detail pane shows the slot, the effective recipe with each field's source, and the target; planned and dangling slots.
- Tests: view-model and snapshot tests per grouping and sort; action dispatch tests.
- Manual checks: `conspectus tui --snapshot` of the Pinned view on the showcase fixture.
- Blockers: the row renderer, slot store, and rebuild engine tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Pinned view lists every slot in slot order by default, grouped by project, with kind and recency alternatives
- [ ] #2 `Enter` performs the kind's primary action, and the menu offers every slot gesture
- [ ] #3 The detail pane shows where each effective recipe field came from
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
