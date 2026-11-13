---
id: CSP-710
title: Delete the superseded sectioned detail model
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 04:39'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: chore
ordinal: 796000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The right pane's explorer replaced the old sectioned detail model, but `src/tui/detail.rs` (1,409 lines, plus 1,180 lines of tests) and most of `src/tui/ui/detail_header.rs` (376 lines) are still in the tree. Production code reads only `NodeDetail.kind` and `ambiguous_muxes_for_group` from them, and the `e` key on the left pane toggles a flag that has no visible effect. Left in place, the model would be inventoried, extracted, or documented by mistake, and it hides which code the TUI actually runs. The inventory flags it as delete-not-extract (slice 07).

Scope: Retarget the two live reads onto the explorer or rows modules. Delete `detail.rs`, `detail_tests.rs`, and the legacy renderer in `detail_header.rs`. Keep `chip_divider_line` (a live section rule, later the framework's `rule`) and move it next to its callers. Remove the `e` binding, its flag, its help row, and its documentation. Update `docs/tui-detail-mockup.md`, and mark any ADR that describes the removed model as superseded in `docs/adr/README.md`.

TUI changes:
- Consistency wins: the code the TUI runs and the code in the tree are the same code
- Regressions or behavior changes: the inert `e` key disappears from the keymap and the help page; it did nothing visible

Tests and checks: Every deleted test is either deleted with the code it covered or rewritten against the retargeted reads; none is dropped silently. `cargo test --all-targets --all-features` passes. `conspectus tui --snapshot` frames are identical before and after for the Sessions, Mux, and Pinned views.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The sectioned detail model (`detail.rs`, its tests, and the legacy renderer in `detail_header.rs`) is deleted, and `chip_divider_line` is kept next to its callers
- [x] #2 Production code that read `.kind` and `ambiguous_muxes_for_group` is retargeted, with its test coverage moved or rewritten rather than dropped
- [x] #3 The `e` binding, its flag, its help row, and its documentation are removed, and the keymap drift tests pass
- [x] #4 `docs/tui-detail-mockup.md` and the ADR index say the model is gone
- [x] #5 `conspectus tui --snapshot` frames for three views match the frames from before the change
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture baseline `conspectus tui --snapshot` frames (showcase fixture, Sessions/Mux/Pinned, several widths, focus states, right-pane walk/drill/e) into /tmp/tuifw-baseline/frames-710-before using the unmodified build. Done before any edit.
2. Re-verify with grep that production code reads only `NodeDetail.kind` and `ambiguous_muxes_for_group`. Findings: `kind` gates and titles the right pane (right_panel.rs), `detail` is otherwise only written (explorer_nav.rs, tree.rs) or read by the unreachable legacy branch and `draw_detail_preview`'s unused parameter.
3. Retarget: the right pane's title, kind label, and empty-state gate read `app.explorer()` (`state.view.kind`); `recompute_detail` / drill / back / empty-selection no longer build or clear a `NodeDetail`, the two `detail = None` sites in tree.rs clear the explorer instead so "nothing selected" stays one state; `draw_detail_preview` drops its unused detail parameter; the unreachable legacy right-pane branch goes. Move `ambiguous_muxes_for_group` (+ its four private helpers) into `src/tui/rows/mod.rs` and its two tests into `rows_tests.rs`; actions.rs calls the rows copy. Rewrite the app_tests that read `app.detail()` against `app.explorer()`.
4. Move `chip_divider_line` and `ChipAnchor` (with corrected docs) into `ui/right_panel.rs`, next to the Related divider; the Preview divider in preview_pane.rs keeps calling it through the ui module glob.
5. Delete detail.rs, detail_tests.rs, ui/detail_header.rs, the `App::detail` / `detail_links_expanded` fields and accessors, `Msg::ToggleLinkedDetails` and `App::toggle_linked_details`, and the legacy renderer tests in ui_tests.rs (`header_field_line_count_*`, `header_zone_height_*`).
6. `e`: the left-pane behavior (a flag toggle with no visible effect, or the status line "detail: no linked entities to expand") goes. The design (docs/design.md Detail Navigation, mockup locked decision 8) documents `e` on the right pane as the explicit Other-zone accelerator, which is live, so the key is rebound straight to `Msg::ExplorerToggleGroup`, dropped on left focus by `remap_for_focus`, and its table help text reworded. Flagged for operator review.
7. Docs: mark ADR 0033 superseded by 0074 (README index + the ADR's Status), ADR 0071 partially superseded (its detail-section part); update docs/tui-detail-mockup.md (note the model is gone, fix stale references) and stale doc comments in explorer.rs / node_show.rs.
8. Capture AFTER frames and diff against BEFORE; run fmt, clippy (workspace, all targets, all features, -D warnings), the isolated full suite, rustdoc with -D warnings, git diff --check. Record deleted line counts and the test-count change in the final summary.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Premise check: production reads of the legacy model were `NodeDetail.kind` (right-pane title, kind label, empty-state gate in `ui/right_panel.rs`) and `ambiguous_muxes_for_group` (`actions.rs`, the `a` key on group rows). Everything else only wrote or rebuilt it, or read it in the unreachable legacy branch of `draw_right_panel`.

The `e` key is not inert everywhere. On the left pane it toggled `detail_links_expanded` (no visible effect). On the right pane `remap_for_focus` turned it into `ExplorerToggleGroup`, the documented Other-zone accelerator (docs/design.md Detail Navigation; tui-detail-mockup locked decision 8; help overlay; status bar "e group"). Only the left-pane behavior, the `ToggleLinkedDetails` message, and the flag were removed; the key is now bound straight to `Msg::ExplorerToggleGroup` and dropped on left focus. Operator to confirm or ask for full removal.

Invariant kept: every site that cleared `detail` now clears `explorer`, so the placeholder shows exactly when it did.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Deleted the sectioned detail model and retargeted its two live reads.

Deleted: `src/tui/detail.rs` 1,409 lines, `src/tui/detail_tests.rs` 1,180, `src/tui/ui/detail_header.rs` 376 (all but `chip_divider_line`/`ChipAnchor`, moved to `ui/right_panel.rs` with corrected docs), 101 lines of legacy renderer tests in `ui_tests.rs`, plus the `App::detail` / `detail_links_expanded` state, `Msg::ToggleLinkedDetails`, the unreachable legacy branch of `draw_right_panel`, and the unused detail parameter of `draw_detail_preview`. Net across 26 files: +648 / -3,397.

Retargeted: the right pane's title, kind label, and empty-state gate read `explorer().view.kind`; the "no selection" paths clear the explorer, so the placeholder shows exactly when before. `ambiguous_muxes_for_group` (+ helpers) moved to `src/tui/rows/mod.rs`; `actions.rs` calls it. `App::recompute_detail` is now `recompute_explorer`.

Tests: suite went from 3483 passed to 3460 passed, 0 failed, 1 ignored (isolated runner). 27 deleted (25 in detail_tests.rs, 2 legacy renderer tests in ui_tests.rs), 4 added: the 2 `ambiguous_muxes_for_group` tests moved to rows_tests.rs, a new test for its checkout/repo scoping, and a keymap test for `e`. The remap test and three app_tests were rewritten against the explorer (the pinned-session test now asserts the explorer lists the pin that targets the session). No test was dropped silently.

Verification: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, full suite via /tmp/tuifw-baseline/run-tests.sh, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, and `git diff --check` all pass (nextest not run separately). `conspectus tui --snapshot` on tests/fixtures/showcase.json (pinned clock): 53 frames over the Sessions, Mux, and Pinned views (default, narrow 70x30, header/status/left/right panes, right-pane focus, walk, drill, drill+back, F, E, e, help overlay) are byte-identical before (/tmp/tuifw-baseline/frames-710-before) and after (.../frames-710-after).

Docs: ADR 0033 superseded by 0074 (index and the ADR's Status); ADR 0071 decision 2 superseded (index reads "Accepted (detail-pane section superseded by 0074)"); ADR 0074 names both; docs/tui-detail-mockup.md says the model is gone and drops its stale references. docs/operations.md and README never mentioned `e`. Historical review docs and ADR bodies that name `detail.rs` in passing were left as records.

**Regressions**
- The `e` key does nothing on the left pane. Before, it flipped an internal flag (no visible effect) or posted "detail: no linked entities to expand" on rows without linked fields.

**Consistency wins**
- The code the TUI runs is the code in the tree: one detail model, one chip-divider implementation documented to match its behavior.

**Deviation to confirm**
- AC 3 says remove "the `e` binding ... its help row". On the right pane `e` is the live Other-zone accelerator (docs/design.md, mockup decision 8, help overlay, status bar), so it was kept (rebound to `ExplorerToggleGroup`) and only the dead left-pane behavior and flag were removed. Removing it entirely is a one-line binding delete plus its help and status-bar mentions and needs a design.md update.

Follow-up filed: CSP-768 (the group-row attach hint says "pick one in the detail pane", but no pane lists those muxes, and had not since the explorer replaced the sectioned detail).
<!-- SECTION:FINAL_SUMMARY:END -->
