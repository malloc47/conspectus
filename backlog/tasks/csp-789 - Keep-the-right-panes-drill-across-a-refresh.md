---
id: CSP-789
title: Keep the right pane's drill across a refresh
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 08:08'
updated_date: '2026-10-30 09:01'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/explorer-design.md
priority: medium
type: bug
ordinal: 875000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The live TUI refreshes every 30 seconds. On every refresh `App::recompute_explorer` rebuilds the right pane for the node the left pane's selection names, and keeps the pane's state (cursor, `Other` fold, Expanded Node Detail, breadcrumb) only when that node is the one the pane is focused on. After `Enter` drills into a neighbor that has no row in the active view (a session's mux, checkout, repo, or process from the Sessions view), the left selection still names the session, so the next refresh throws the drill away: the pane jumps back to the session with an empty breadcrumb. A throwaway reducer probe written for the explorer design (CSP-755) reproduced it: drill from `app_for_explorer` into the mux, deliver `SetData` with the same snapshot, and the pane is on the session at depth 0. Separately, no test covers `Backspace` to a frame whose node has gone (`explorer_back` drops that hop with a warning and the next press goes on), which the framework's back rule will change.

The generic explorer's refresh rule (docs/plans/tui-framework/explorer-design.md, section 2.5) reloads the frame the pane shows and keeps the drill while the left selection is unchanged, so the defect is fixed here first, ahead of the extraction (ADR 0151: a defect is fixed before the extraction that would otherwise paper over it).

Scope: Conspectus only. The right pane remembers which left-pane selection it reflects (set when it opens, after the mirror moves the left pane, and after Backspace restores it); a refresh or projection change that leaves that selection alone reloads the focused node in place and keeps the breadcrumb, fold, detail toggle, and cursor; any other selection change opens the pane on the new node as today. When the focused node is gone after a refresh, the pane opens on the left selection's node. A test pins Backspace to a gone node as it behaves today.

TUI changes:
- Consistency wins: a drill survives the periodic refresh, a projection change, and `r`
- Regressions or behavior changes: none expected besides the fix

Tests and checks: reducer tests for each criterion; `conspectus tui --snapshot` cannot show it (snapshot mode's `r` runs real discovery), so the reducer tests are the check.

Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A reducer test drills into a neighbor that has no row in the active view, delivers a refresh with the same snapshot, and finds the pane still on the neighbor with its breadcrumb, fold, detail toggle, and cursor; it fails before the fix
- [x] #2 Selecting another row in the left pane still opens the pane on that row's node with an empty breadcrumb
- [x] #3 A refresh that removes the focused node opens the pane on the left selection's node
- [x] #4 A test pins Backspace to a node that is gone: it says so, and the next Backspace goes on from the frames that remain
- [x] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Tests first in src/tui/app_tests.rs: the design probe as a permanent reducer test (drill from a session into a mux with no row in the Sessions view, set the Other fold, the Expanded Node Detail toggle and the cursor, deliver SetData with the same snapshot, expect all kept), parked with an owned `#[ignore]` until the fix; a pin of Backspace to a gone frame (drill session > mux > session, refresh without the mux, Backspace warns and drops the hop, the next press restores the session and the left selection), passing today.
2. Fix in src/tui/app.rs and app/explorer_nav.rs: an `App` field for the left selection the right pane reflects, set by `recompute_explorer`, by `mirror_left_pane_to` after it selects the drilled row, and by `explorer_back` after it restores a frame's selection. In `recompute_explorer`, when the selection equals it and the pane is drilled, reload the focused node in place and keep the breadcrumb, fold, toggle and cursor (reseat by row key; one `ExplorerState` reload helper shared with the existing same-node path); when the focused node no longer builds, open on the selection's node. `set_selection` (a programmatic jump: pickers, quick select, regroup, follow, undo) forgets it first, so a pick of the already-selected row opens on that row's node as today.
3. More reducer tests: projection change (filter/sort rebuild in place) and `r` (SetData) keep the drill; selecting another row opens on it with an empty breadcrumb; a refresh that removes the focused node opens on the left selection's node; un-ignore the probe test.
4. Docs: ADR 0188 status line records what CSP-789 changed (ADR 0074 decision 6 narrowed) and the ADR index; docs/design.md / docs/operations.md / CHANGELOG.md if a documented behavior changes; operator-review.md judgment calls.
5. Checks: fmt, clippy, full isolated suite, cargo doc, git diff --check; finalize the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Slice 1 (test): the design probe as `a_refresh_keeps_a_drill_into_a_neighbor_the_view_has_no_row_for`, parked with `#[ignore = "backlog CSP-789: ..."]` (the consistency suite's convention for a test that fails today) so every commit passes; run with --include-ignored it failed with the pane back on the session (AgentSession abc) instead of the mux. Three pins that pass today: moving the left selection, a refresh removing the drilled node, Backspace to a gone frame (drill session > mux > second session, refresh without the mux).

Slice 2 (fix): `App::explorer_reflects`; `recompute_explorer` swaps it for the current selection and, when they match and the pane is drilled, calls `reload_drill` (focused node rebuilt, `ExplorerState::reload` keeps breadcrumb, fold, detail toggle, cursor by key); otherwise today's path. Mirror and Backspace set it after they move the left selection. `set_selection` clears it so a jump onto the selected row opens on it (the existing `explorer_state_resets_when_left_tree_selection_changes` encodes that and still passes). The reload applies only to a drilled pane, so an undrilled pane still follows its row's node if the row's node changes. New tests: sort and filter rebuild keeps the drill; a pick of the selected row opens on it. All 1504 `tui::` unit tests pass.

Found: every refresh resets the Preview's scroll (`recompute_explorer` calls `reset_preview_scroll` first); out of scope, filed as CSP-792.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The right pane's drill now survives a refresh. `App` keeps `explorer_reflects`, the left selection the pane reflects: set by `recompute_explorer` whenever it opens or reloads the pane, by `mirror_left_pane_to` after it selects a drilled node's row, and by `explorer_back` after it restores a frame's selection. When `recompute_explorer` runs with the selection still on that row and the pane drilled, `reload_drill` rebuilds the focused node and `ExplorerState::reload` keeps the breadcrumb, the `Other` fold (while the zone has rows), Expanded Node Detail, and the cursor (reseated by row key); the same helper now serves the same-node path. When the focused node no longer builds, the pane opens on the selection's node. `set_selection`, the path of every jump to a row, forgets the field first, so a pick of the already-selected row opens on it as before (judgment call, recorded in operator-review.md). Conspectus only; no framework code.

Tests (src/tui/app_tests.rs): the design spike's probe, committed first with an owned `#[ignore]` and failing (pane back on the session at depth 0), un-ignored by the fix; a sort and filter rebuild keeps the drill; a pick of the selected row opens on it; moving the left selection opens on the new row with an empty breadcrumb; a refresh that removes the drilled node opens on the left selection's node; Backspace to a frame whose node is gone warns, drops the frame, and the next press restores the older frame and the left selection. `conspectus tui --snapshot` cannot show this (snapshot mode's `r` runs real discovery), so the reducer tests are the check.

Docs: docs/design.md (the drill rule), ADR 0188 status (what landed), ADR 0074 status and the ADR index (narrowed by 0188), explorer-design.md step 0 (as landed), CHANGELOG.md (Fixed), operator-review.md. Follow-up filed: CSP-792 (a refresh returns the Preview to its anchor).

Checks (the steps of `just check`, tests through the tmux-isolated runner): cargo fmt --check pass; clippy --workspace --all-targets --all-features -D warnings pass; full suite 4510 passed, 0 failed, 1 ignored (first run hit the known `discovery::forge` timing flake `a_cancelled_hosts_lookup_returns_promptly_with_no_answer`, clean on rerun); RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps pass; git diff --check pass.

**Regressions**
- None.

**Consistency wins**
- A drill survives the periodic refresh, `r`, and a filter or sort change, with its breadcrumb, `Other` fold, Expanded Node Detail, and cursor.
- A left-pane key that does not move the selection (`j` on the last row, `Home` on the first) leaves a drill alone; it dropped it.
<!-- SECTION:FINAL_SUMMARY:END -->
