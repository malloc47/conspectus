---
id: CSP-702
title: Remember expanded groups across TUI restarts
status: To Do
assignee: []
created_date: '2026-10-24 03:20'
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
The TUI already restores the last view and each view's filter, grouping, sort, and preview settings from `$XDG_STATE_HOME/conspectus/tui-state.json` (`src/tui_state.rs`, `App::restore_persisted_state` and `App::build_persisted_state` in `src/tui/app.rs`). It does not restore which group rows were expanded. Each view keeps an `expanded: BTreeSet<RowId>` in its `ViewStateSlot`, but `PersistedViewSlot` has no field for it, so every new `conspectus tui` starts with every group collapsed. The operator has to re-open the same workspaces, repos, and muxes each time.

Things to handle: group `RowId`s are keyed on graph `NodeId`s, so a persisted id can point at a node that is gone or renamed after a rebuild. The set can also grow without limit as groups come and go. This file is a rebuildable cache (ADR 0143, category 2), so this needs no new write path.

`docs/operations.md` (the "TUI state" section) still describes the file as holding only `last_view`, so it is already out of date.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 After a group row is expanded in a view and the TUI exits, the next `conspectus tui` shows that group expanded in that view; groups that were collapsed stay collapsed
- [ ] #2 Expansion is remembered per view, the same way filter and grouping are, and switching views during a run keeps each view's expanded set
- [ ] #3 A persisted group whose node no longer exists is ignored without an error, and it is dropped from the file on the next write so the set does not grow without limit
- [ ] #4 A state file written by an earlier build (with no expansion field) still loads and starts with all groups collapsed; unknown fields still round-trip
- [ ] #5 `--no-resume-view` and `--snapshot` do not restore persisted expansion, so snapshots stay deterministic
- [ ] #6 Writes stay atomic and skip-on-unchanged; expanding or collapsing a group does not write the file on every keypress if the existing write policy batches it
- [ ] #7 Tests cover the round trip, a stale node id, an old-format file, and the opt-out flags; `tests/cli_tui_state_invariants.rs` still passes
- [ ] #8 The "TUI state" section of `docs/operations.md` describes everything the file now holds, including expanded groups
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
