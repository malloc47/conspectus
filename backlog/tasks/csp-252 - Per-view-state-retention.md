---
id: CSP-252
title: Per-view state retention
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies:
  - CSP-250
  - CSP-251
  - CSP-254
ordinal: 448000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: introduce `ViewStates` map on `App`, keyed by `View`,
  carrying `(filters, grouping, expanded, selection, left_scroll)`.
  On view switch, save the active slice and load the target
  slice; first-time entries seed from
  `[tui.views.<name>]` config defaults. Sort stays a top-level
  `App` field per ADR 0031.
- Tests: reducer tests for switch-and-return state retention
  (filters survive `1 → 2 → 1`), per-view selection retention
  across refreshes, fresh-view default seeding from config.
- Blockers: `CSP-250`, `CSP-251`, `CSP-254`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped — `App::view_states: BTreeMap<View,
ViewStateSlot>` with `switch_to_view` saving the active slot
and loading the target slot. Fresh entries seed from
`[tui.views.<name>]` config defaults. Reducer tests pin
switch-and-return filter retention and per-view selection.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-003`
