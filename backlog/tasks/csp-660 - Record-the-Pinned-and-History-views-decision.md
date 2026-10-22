---
id: CSP-660
title: Record the Pinned and History views decision
status: Done
assignee:
  - '@claude'
created_date: '2026-10-22 01:10'
updated_date: '2026-10-22 02:34'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-657
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0073-node-kind-visual-identity.md
  - docs/adr/0131-pr-titles-and-the-pr-row-layout.md
priority: high
type: docs
ordinal: 718000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator wants pinned entities in their own Pinned view, a peer of Sessions, Mux, PRs, and the others, rather than a Pins group at the top of each view, and wants history browsable in a History view. Both views list different kinds together, which no current view does, so the row layout needs settling before the renderer is built.

- Scope: the shared row layout `<kind> <label> <status> <age> <content>` with fixed columns padded over the whole tree and content truncated last (ADR 0131's rule); status values and content per kind; dimming for gone, missing, and planned rows instead of a new glyph; the kind glyph shown for every kind in these views (amending ADR 0073's suppression for sessions); grouping (project, kind, none); sort (slot order by default in Pinned, with recency available; last seen in History); expansion; primary `Enter` actions and menus; the Clean up modal; ghost muxes in the Mux view with header counts; the pin marker and `pinned only` filter in native views; removal of the Pins groups, placeholder rows, Pins modal, and `N` / `B` / `b` / `A` pin keys, with `N` becoming mux launch with worktree.
- Tests: docs-only; `git diff --check`.
- Manual checks: none.
- Blockers: the pin slots ADR.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A new ADR records the column spec, statuses per kind, grouping and sort options, actions, and the Mux and native-view changes
- [x] #2 The ADR amends ADR 0073 for kind glyphs in views that mix kinds and references ADR 0131's alignment rule
- [x] #3 The ADR includes a mockup of each view at a typical width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Write ADR 0144 from the plan's views section, adding the row action menu (no generic one exists today) and key assignments.
2. Index it and amend ADR 0073's status.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Recorded ADR 0144 (Pinned and History views): the `<kind> <label> <status> <age> <content>` layout with whole-tree padding (ADR 0131), statuses and content per kind, dimming, grouping and sort, `Enter` actions, a new `.` row action menu with `p` as the pin accelerator, the Clean up modal, ghost muxes in the Mux view with header counts, the native-view pin marker and `pinned only` filter, and removal of the Pins groups, placeholders, modal, and keys. Mockups of the Pinned, History, and Mux views. Amends ADR 0073. Verified with `git diff --check`.
<!-- SECTION:FINAL_SUMMARY:END -->
