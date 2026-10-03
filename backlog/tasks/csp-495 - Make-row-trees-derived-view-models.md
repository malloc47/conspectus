---
id: CSP-495
title: Make row trees derived view-models
status: Done
assignee: []
created_date: '2026-07-02 02:16'
labels:
  - h-tui
milestone: m-11
dependencies: []
ordinal: 104000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-01. `App` now owns `active_view`; `RowTree` is a
  pure derivation of `(snapshot, view, grouping, filter, sort,
  cwd)` via `build_tree_for_view(TreeInputs::from_app(...))`.
  `apply_controls_action_and_refresh` is now
  `apply_controls_action_and_rebuild` — mutates App, emits
  `Msg::SetTree` against the held snapshot, no discovery.
  `RunConfig` is initial-values-only; `apply_controls_action` /
  `switch_to_view` / `force_recency_for_flat_sessions` /
  `restore_persisted_state` no longer mirror back to it. Also
  fixed the timer-refresh race: the discovery worker's result
  path now builds against App projection state. Regression net:
  four `projection_zero_discovery::*` tests plus the existing
  TUI snapshot suite. `CSP-467` (`SnapshotIndex`) is a follow-on
  optimization; CSP-495 does not depend on it.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-TUI-001`
