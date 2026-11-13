---
id: CSP-702
title: Remember expanded groups and the selected row across TUI restarts
status: Done
assignee:
  - '@claude'
created_date: '2026-10-24 03:20'
updated_date: '2026-11-12 02:16'
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
- [x] #1 After a group row is expanded in a view and the TUI exits, the next `conspectus tui` shows that group expanded in that view; groups that were collapsed stay collapsed
- [x] #2 After a row is selected in a view and the TUI exits, the next `conspectus tui` opens that view with the same row selected and scrolled into view
- [x] #3 Expansion and selection are remembered per view, the same way filter and grouping are, and switching views during a run keeps each view's expansion set and selection
- [x] #4 A persisted group whose node no longer exists is ignored without an error, and it is dropped from the file on the next write so the set does not grow without limit
- [x] #5 When the persisted selected row no longer exists, the cursor lands on the deepest surviving ancestor group of the path stored with it, and on the first row when none survives; neither case is an error or a warning
- [x] #6 A restored selection is never on a hidden row: groups on its path are expanded and the pane scrolls to it
- [x] #7 The Search view persists neither its expansion nor its selection; the right pane's drill and the preview scroll are not persisted
- [x] #8 A state file written by an earlier build (with no expansion or selection field) still loads and starts with all groups collapsed and the first row selected; a file with a malformed or unknown-kind row id loads with that view's expansion and selection ignored; unknown fields still round-trip
- [x] #9 `--no-resume-view` and `--snapshot` do not restore persisted expansion or selection, so snapshots stay deterministic
- [x] #10 Writes stay atomic and skip-on-unchanged; expanding, collapsing, or moving the cursor does not write the file on every keypress if the existing write policy batches it
- [x] #11 Tests cover the round trip of expansion and selection, a stale group id, a stale selected row with a surviving ancestor, a stale row with none, an old-format file, a malformed row id, the hidden-row case, and the opt-out flags; `tests/cli_tui_state_invariants.rs` still passes
- [x] #12 The "TUI state" section of `docs/operations.md` describes everything the file now holds, including expanded groups and the selected row, and the fallbacks
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. tuikit: add `TreeState::select_at(rows, index)` (reveal the row's ancestors and put the cursor on that copy) and make `tree::ancestors` public; unit tests and control-page rows.
2. tui_state.rs: per-view `expanded` (list of row keys) and `selected` ({row, path}) fields, a top-level `row_keys` encoding version, per-entry lenient reading, per-view `extra` round-trip; one injective string encoding of the persisted RowId kinds (Search-only kinds not encoded).
3. App: read the tree state per view into pending restores (skipped under `--no-resume-view`, never for Search); apply once when the view first lists rows (expanded keys matched against the rebuilt rows, selection by row then deepest surviving ancestor on the stored path then first row, path revealed); a regroup cancels it.
4. Writes: build_persisted_state computes each view's expansion (kept only for ids a row of that view's last tree carries) and selection path; inactive views keep what was computed when they were left, unvisited views keep what was read. Same write policy (view switch, settings, exit); also write before a terminal hand-off and when the loop ends in an error.
5. CLI: carry `--no-resume-view` into RunConfig; snapshot mode never restores or writes.
6. Tests first: tui_state unit tests, App-level restore tests through the live entry point, CLI snapshot invariance test; then docs (operations.md TUI state, CHANGELOG, ADR 0202 amending 0031, design.md) and operator-review rows.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the expanded set moved. `ViewStateSlot` (`src/tui/app.rs`) keeps each view's tree state as `tree: TreeState<RowId>` from `tuikit::tree` (CSP-750, ADR 0182); the expanded group ids are `TreeState::expanded()`, and a restore is `TreeState::set_expanded(..)`. `TreeState` keeps whatever ids it is given, so dropping ids whose node is gone (AC #3) is the host's job when it builds `PersistedViewSlot` (`src/tui_state.rs`): keep only the ids a row of the rebuilt tree carries. `App::restore_persisted_state` and `App::build_persisted_state` are still the two ends.

Implemented in three commits: the framework's `TreeState::select_at` and public `tree::ancestors`; the host restore (`src/tui_state.rs` row keys and per-view `expanded`/`selected`, `src/tui/app/restore.rs`, `RunConfig::resume_view`, `runtime::resumed_app`, writes before a terminal hand-off and on a loop error); docs (operations.md TUI state, CHANGELOG, ADR 0202 amending 0031, design.md, operator-review rows).
Validation: full isolated suite 4920 passed, 0 failed, 1 ignored (an earlier run stopped at a tmux-server flake in `cli_history::rebuild_plan_of_a_running_mux_is_a_pasteable_script`, which passed on an isolated rerun); fmt; clippy workspace all-features and `-p tuikit`; rustdoc -D warnings; `cargo build --examples`; `just tuikit-features`; tuikit independence and doc_sync; `git diff --check`. The new restore tests fail with the restore hook disabled (12 of 18).
Filed CSP-835 for whether --no-resume-view should stop writing the file (its --help claimed it did not write; it always has).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Each view but Search now reopens with the rows it had expanded and the row it had selected. `tui-state.json` keeps, per view, `expanded` (row keys) and `selected` (`{row, path}`, path top-level first), with a top-level `row_keys: 1` naming the key encoding. A key is the row's kind, a colon, and its node id text, record or pin id, label, or slug (`group:workspace:/src`, `session:agent_session:…`, `nested:<pin>\n<node>`). Keys are compared as text against the rows a later run lists, never decoded; an id whose text reads back as another gets no key; Search's hit and pane rows get none.

A view's saved tree applies once, the first time the view lists rows: saved folds for listed rows only; cursor on the saved row, else the deepest path row still listed, else the first row; groups above it opened; the copy under the saved path when a row is listed twice (new `TreeState::select_at`). A regroup before then drops it, a jump into the view wins, later refreshes never reapply it. Writes keep only listed rows' folds (gone groups leave the file); a view left earlier is written as it was left, an unvisited one unchanged. Write points are unchanged plus two: before a terminal hand-off and when the loop ends in an error. `--no-resume-view` reopens no rows; `--snapshot` reads and writes nothing. Reading is lenient per entry; an old file opens as before; unknown per-view fields now round-trip.

Verified with unit tests (`src/tui_state_tests.rs`, `src/tui/app/restore_tests.rs` through `runtime::resumed_app` and `set_static_data` over the showcase fixture, the Search case in `consistency_tests/search_view.rs`), a CLI test that `tui --snapshot` neither reopens nor rewrites a seeded file, the full isolated suite (4920 passed, 0 failed, 1 ignored), clippy, rustdoc, `just tuikit-features`, `cargo build --examples`. Docs: operations.md "TUI state", CHANGELOG, ADR 0202 (amends 0031), design.md. Follow-up: CSP-835.

**Regressions**
- On the starting view, a remembered row now beats the launch directory's group as the first row selected; the launch directory still picks it for a view with nothing remembered.
- A group expanded by default and collapsed by hand stays collapsed on the next run.

**Consistency wins**
- Every view, not just the starting one, comes back as left: its folds, its selection scrolled into view, and the groups above the selection open.
- `--no-resume-view`'s help now says what it does: it skips the remembered view and rows, not the writes.
<!-- SECTION:FINAL_SUMMARY:END -->
