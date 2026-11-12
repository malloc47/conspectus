---
id: CSP-702
title: Remember expanded groups and the selected row across TUI restarts
status: To Do
assignee: []
created_date: '2026-10-24 03:20'
updated_date: '2026-11-12 01:02'
labels:
  - tui
milestone: m-23
dependencies: []
priority: medium
type: feature
ordinal: 778000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI already restores the last view and each view's filter, grouping, sort, and preview settings from `$XDG_STATE_HOME/conspectus/tui-state.json` (`src/tui_state.rs`, `App::restore_persisted_state` and `App::build_persisted_state` in `src/tui/app.rs`). It does not restore which group rows were expanded or which row was selected. Each view keeps its expansion set and selection in the tree state of its `ViewStateSlot` (`tuikit::tree::TreeState<RowId>` since the TUI framework work), but `PersistedViewSlot` has no field for either, so every new `conspectus tui` starts with every group collapsed and the cursor on the first row. An operator who drops in and out of the TUI has to re-open the same workspaces, repos, and muxes and find their place again each time.

Things to handle: row ids are keyed on graph `NodeId`s, pin ids, and history record ids, so a persisted id can point at something that is gone or renamed after a rebuild, a daemon restart, or a pin removal. The expansion set can also grow without limit as groups come and go. This file is a rebuildable cache (ADR 0143, category 2), so this needs no new write path.

Selection needs a fallback chain, because the exact row may not exist when the operator comes back: the row itself if it is still listed; otherwise the deepest surviving ancestor group on the path the row was reached by (so a session whose group is still there lands on that group, not at the top of the tree); otherwise the first row, as today. To make the ancestor step possible the file stores, with the selected row, the ids of the groups above it. The restored selection must be visible: the pane scrolls to it, and any group on its path that the saved expansion set did not cover is expanded rather than leaving the cursor on a hidden row.

Not everything is stable enough to remember: the Search view's rows are results of a query that is not itself restored, so neither its expansion nor its selection is persisted. The right pane's drill and the preview's scroll offset are not persisted either.

`docs/operations.md` (the "TUI state" section) still describes the file as holding only `last_view`, so it is already out of date.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 After a group row is expanded in a view and the TUI exits, the next `conspectus tui` shows that group expanded in that view; groups that were collapsed stay collapsed
- [ ] #2 After a row is selected in a view and the TUI exits, the next `conspectus tui` opens that view with the same row selected and scrolled into view
- [ ] #3 Expansion and selection are remembered per view, the same way filter and grouping are, and switching views during a run keeps each view's expansion set and selection
- [ ] #4 A persisted group whose node no longer exists is ignored without an error, and it is dropped from the file on the next write so the set does not grow without limit
- [ ] #5 When the persisted selected row no longer exists, the cursor lands on the deepest surviving ancestor group of the path stored with it, and on the first row when none survives; neither case is an error or a warning
- [ ] #6 A restored selection is never on a hidden row: groups on its path are expanded and the pane scrolls to it
- [ ] #7 The Search view persists neither its expansion nor its selection; the right pane's drill and the preview scroll are not persisted
- [ ] #8 A state file written by an earlier build (with no expansion or selection field) still loads and starts with all groups collapsed and the first row selected; a file with a malformed or unknown-kind row id loads with that view's expansion and selection ignored; unknown fields still round-trip
- [ ] #9 `--no-resume-view` and `--snapshot` do not restore persisted expansion or selection, so snapshots stay deterministic
- [ ] #10 Writes stay atomic and skip-on-unchanged; expanding, collapsing, or moving the cursor does not write the file on every keypress if the existing write policy batches it
- [ ] #11 Tests cover the round trip of expansion and selection, a stale group id, a stale selected row with a surviving ancestor, a stale row with none, an old-format file, a malformed row id, the hidden-row case, and the opt-out flags; `tests/cli_tui_state_invariants.rs` still passes
- [ ] #12 The "TUI state" section of `docs/operations.md` describes everything the file now holds, including expanded groups and the selected row, and the fallbacks
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the expanded set moved. `ViewStateSlot` (`src/tui/app.rs`) keeps each view's tree state as `tree: TreeState<RowId>` from `tuikit::tree` (CSP-750, ADR 0182); the expanded group ids are `TreeState::expanded()`, and a restore is `TreeState::set_expanded(..)`. `TreeState` keeps whatever ids it is given, so dropping ids whose node is gone (AC #3) is the host's job when it builds `PersistedViewSlot` (`src/tui_state.rs`): keep only the ids a row of the rebuilt tree carries. `App::restore_persisted_state` and `App::build_persisted_state` are still the two ends.
<!-- SECTION:NOTES:END -->
