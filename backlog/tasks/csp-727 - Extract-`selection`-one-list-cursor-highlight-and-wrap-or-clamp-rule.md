---
id: CSP-727
title: 'Extract `selection`: one list cursor, highlight, and wrap-or-clamp rule'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 19:21'
labels:
  - tui-framework
  - tuifw-foundation
milestone: m-25
dependencies:
  - CSP-721
  - CSP-722
  - CSP-724
documentation:
  - docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
  - docs/adr/0151-tui-framework-control-set-and-house-rules.md
priority: high
type: enhancement
ordinal: 813000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Lists disagree on how selection looks and behaves. Four highlight looks exist: `►` plus BOLD (menus), `> ` plus REVERSED (Controls, Picker, Search, forms), accent only, and the themeable `selection_active` (Messages, left pane). At least 6 widgets implement the highlight independently, the unfocused explorer shows no cursor, and menus clamp at the ends while longer lists wrap. House rules 1, 2, and 10 settle these. Doing this once, ahead of the control extractions, means each later control story does not change the look again.

Scope: Create `tuikit::selection` with the cursor marker (`marks::POINTER` in a 2-cell gutter), the full-row highlight from `selection_active` and `selection_inactive`, ghost and pending row styles, and the wrap-or-clamp stepping rule (short choice lists wrap; scrollable content clamps). Migrate every in-place site: Controls, Picker, Search, forms, the menus, Messages, the explorer, and the left pane.

TUI changes:
- Consistency wins: one selection look in every list; the marker keeps selection visible without colour; the unfocused explorer shows its cursor; menus wrap like the other choice lists
- Regressions or behavior changes: menus, Controls, Picker, Search, and the forms change their cursor glyph and highlight; menus now wrap at the ends instead of stopping; the right pane gains a visible inactive cursor

Tests and checks: `conspectus tui --snapshot` of every list at 80 and 40 columns, with focus on and off. A test fails on a hand-written `REVERSED` cursor in framework source.

Blockers: The theme, glyph, and scroll extractions.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `tuikit::selection` provides the marker, the active and inactive highlight, the ghost and pending styles, and the wrap-or-clamp rule
- [x] #2 Every list and menu uses it, and no list hand-writes a `REVERSED` cursor or a `> ` or `►` literal
- [x] #3 Short choice lists wrap and scrollable content clamps, documented in the reference page
- [x] #4 The right pane shows an inactive cursor when unfocused
- [x] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #6 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #7 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `tuikit::selection`: a `Cursor` (off, active, inactive), the 2-cell gutter drawing `marks::POINTER`, the full-row highlight from `selection_active`/`selection_inactive` (filled spans such as chips and input fills keep their look), `row` that joins them, the ghost and pending looks moved from the left pane, and `Ends` (wrap or clamp) over `scroll::wrap_step`/`clamp_step`. Tests: marker and pair, filled spans, a colorless theme where the marker alone shows the row, wrap and clamp at both ends. The raw-style guard gains a `Modifier::REVERSED` needle. Reference page, module map, contribution step, README.
2. Menus (row, mux, worktree list) and Messages: rows through `selection::row`; menus step with `Ends::Wrap`, Messages (a log) with `Ends::Clamp`.
3. Controls, Picker, Search (quick select, content, options pane), and the checklist under Controls and Clean up: rows through the module; the checklist draws its own rows (the upstream crate keeps only its state until its own story); cursors step with `Ends::Wrap`.
4. Forms: `form::prefix` and the mux launch toggle row draw the gutter and highlight; field rings step with `Ends::Wrap`.
5. Explorer: the marker in its 2-cell lead, the whole row highlighted, the inactive pair when the pane is unfocused; tree and explorer step with `Ends::Clamp`. Left pane: highlight, ghost, and pending through the module with its look unchanged (no gutter).
6. Tests per control (marker and highlight, focused vs unfocused pair, wrap vs clamp), a consistency test over every list modal, and deliberate updates to tests and snapshots that encoded the old looks.
7. Docs: ADR for the selection rule, ADR index, design.md, CHANGELOG, operations.md, tui-columns.md glyph table, operator review notes.
8. Before/after `--snapshot` frames of every list at 100x30, 60x20, 40x16; fmt, clippy, full isolated suite, rustdoc, diff check.

9. Snapshot mode cannot reach the Messages list with entries: add a dev-only --snapshot-message flag (and age Messages against the render clock) before capturing.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Slices: tuikit::selection module (74799e2); --snapshot-message so Messages can be captured with entries (d3c6543); menus and Messages (285cb00); Controls, picker, search, checklist (09f2790); forms (46ed800); explorer and left pane (e1564fc); consistency tests (93c10f2); ADR 0159 and docs (dc857ed). Each intermediate commit passes `cargo check --workspace --all-targets --all-features` and the `tui::` library tests.

Frames: /tmp/tuifw-baseline/frames-727-before and -after (capture-727.sh: 83 captures at 100x30, 60x20, 40x16). classify727.py counts 90 cells of `>` turning into the pointer, 210 of the pointer appearing or moving in a gutter (mostly the right pane's unfocused cursor row), 12 labels now cut with `…` (checklist and Clean up at 40 columns), and 62 other cells, all from Messages' columns moving two cells right for the gutter, the conflict mark moving after the explorer's gutter, and the history row menu wrapping instead of stopping at its end. 24 captures are identical (the left pane at its end, empty Messages, empty quick select at narrow sizes); 10 differ only in styling (menus and forms at 40x16 where the right pane is hidden).

Checks: fmt clean; clippy (workspace, all targets, all features, -D warnings) clean; isolated full suite 3787 passed, 0 failed, 1 ignored; nextest 3787 passed, 1 skipped; RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps clean; diff check clean; tuikit independence guard passes.

Follow-up filed: CSP-775 (the left pane tree's cursor marker, after the column schema).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::selection` (ADR 0159): `Cursor` (off, active, inactive), the `marks::POINTER` marker in a 2-cell gutter (`gutter`, `GUTTER_WIDTH`), the full-row highlight from `selection_active`/`selection_inactive` with filled spans (chips, field fills) kept (`highlight`, `row`), the ghost and pending looks moved from the left pane (`ghost_style`, `ghost`, `pending`), and `Ends::{Wrap, Clamp}` over the scroll steps. Reference page `crates/tuikit/docs/controls/selection.md`; the raw-style guard now fails on `Modifier::REVERSED` in framework source. Every list moved onto it: the row, mux, and worktree menus, Messages, Controls, the picker, quick select, content search and its options, the checklist under Controls and Clean up (drawn by the module; the upstream crate keeps only its state), the three forms, the explorer, and the left pane (highlight, ghost, pending; no gutter). No `> `, `►`, or hand-written `REVERSED` cursor remains; a host consistency scan keeps it so. Snapshot mode gained `--snapshot-message` so Messages can be captured with entries.

Verified with `conspectus tui --snapshot` before/after captures of every list (83 states at 100x30, 60x20, 40x16 in /tmp/tuifw-baseline/frames-727-*), unit and buffer tests per control (marker and highlight, focused vs unfocused pair, wrap vs clamp at both ends, a colorless theme where the marker alone finds the row), a cross-modal consistency module, the isolated full suite (3787 passed, 0 failed, 1 ignored), nextest, clippy, fmt, rustdoc, and the diff check.

**Regressions**
- Every list changes its cursor glyph and highlight: the menus were bold-only, Controls, the picker, the search lists, and the forms showed `>`; all now show `►` with the row reversed and bold.
- The menus (row, mux, worktree) wrap at their ends instead of stopping; Down past the last entry lands on the first.
- The right pane gains a visible cursor row while the tree has focus (the marker in bold), and with focus reverses the whole row rather than one value.
- Messages' entries move two cells right for the gutter.
- The search lists' cursor row keeps its dimmed context under the highlight (it used to drop the dim) and gains bold.
- A form's focused row is reversed on its label and suffix around the field's unchanged fill.
- A conflict row's `▲` in the explorer's Other zone moves two cells right (after the gutter); the verb column holds.
- The armed row menu entry's marker is no longer red (its label stays red).
- Checklist and Clean up labels too long for the row now end in `…` instead of being cut silently.

**Consistency wins**
- One selection look in every list, from the theme, so `[tui.theme] selection_active`/`selection_inactive` now reach every list.
- The marker keeps the cursor row visible without color or reverse video.
- The unfocused right pane shows its cursor (house rule 10); Controls' list shows the inactive pair behind its open checklist or editor.
- Menus wrap like the other short choice lists; the tree, explorer, and Messages clamp.
- The left pane's look is unchanged; CSP-775 gives it the gutter after the column schema.

Operator review: the left pane draws no gutter yet, the highlight is not padded to the list width, dim text stays dim under the highlight, and the marker takes the row's style, not the accent (docs/plans/tui-framework/operator-review.md).
<!-- SECTION:FINAL_SUMMARY:END -->
