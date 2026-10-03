---
id: CSP-501
title: Drop the Union view and hide the PRs and Forks views from the TUI
status: Done
assignee: []
created_date: '2026-07-28 03:30'
labels:
  - h-view
milestone: m-18
dependencies: []
ordinal: 526000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `VIEW_OPTIONS` (`src/tui/widgets/controls.rs`) is the single
  source of truth for both the controls-overlay View section and the
  `[` / `]` view-cycle accelerator. Reduce it to `[Sessions, Mux]` so
  Union / PRs / Forks no longer surface in the interactive UI, which
  was causing confusion. Keep the `View` enum, per-view config slices,
  row builders, and the `conspectus table union|prs|forks` CLI
  projections intact — this is a UI-surface hide, not a model
  removal, so the work is reversible when those views mature.
- Tests: update the controls-overlay renderer snapshots and the
  view-cycle tests to the shortened option set; confirm no default
  view/grouping resolves to a now-hidden view.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): landed in `b84aa05`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-VIEW-001`
