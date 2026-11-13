---
id: CSP-754
title: 'Extract `PropertyGrid`: property, link, and disclosure rows'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 07:43'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-723
  - CSP-727
  - CSP-713
  - CSP-746
  - CSP-710
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 840000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The right pane's node fields (label column, value, placeholder, annotation, glyph, copy and inspect) and Related rows (validated, quiet, alert, and inert tones) are a property grid. Highlight extent differs in four ways (the explorer ignores `theme.selection_*`, highlights only the value span on field rows, and shows no cursor when unfocused), 9 glyph sites bypass the glyph table, and `core_fields.get(index)` is the copy defect. Disclosure rows exist in both panes (inventory slice 07).

Scope: Create `PropertyGrid` with `PropertyRow` (label, value, dim, hint, annotation, glyph), a `LinkRow` family (indent, prefix, verb column, glyph, middle-truncated label, meta, tone), and `DisclosureRow`, with copy and inspect outcomes (inspect opens the value in the pager modal). Use the shared selection pair. Domain content (`FieldValue`, link semantics, provider chips) stays in Conspectus.

TUI changes:
- Consistency wins: one highlight extent; the unfocused cursor is visible; glyphs come from the glyph table
- Regressions or behavior changes: the highlight on field rows covers the whole line instead of only the value

Tests and checks: The explorer tests (`explorer_tests.rs`) and right-pane tests pass or are updated with a note. `--snapshot` of a node with many fields, `F` on and off, focused and unfocused.

Blockers: Text, selection, the copy defect, and the pager modal.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `PropertyGrid` provides property, link, and disclosure rows with copy and inspect outcomes
- [x] #2 Highlight extent follows the shared selection rule in both focus states
- [x] #3 The 9 hard-coded glyph sites use the glyph table
- [x] #4 Domain value types and link semantics stay in Conspectus
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
1. Capture before frames of the right pane (node kinds with many fields, F on/off, a cut value under the cursor, Related rows of every tone incl. conflicts, alternates, unresolved, Other folded/unfolded, edge meta, the Preview of a related row; focused and unfocused) at 160x40, 100x30, 60x20 into /tmp/tuifw-baseline/frames-754-before.
2. Framework: `tuikit::property_grid` with `PropertyRow` (label column with a gap for long labels, value as text or spans, cut in cells at the end or middle with the truncated hint naming the host's inspect key, placeholder, hint, annotation, glyph), `LinkRow` (indent, verb column, glyph or the no-value mark, middle-cut label, meta kept and wrapping when the label would fall under 8 cells, `Tone` primary/quiet/alert/inert with `Tones`), `DisclosureRow` (the tree's mark via `marks::disclosure`, label, count, tallies), `Row`, `PropertyGrid` (rows and lines that are no row through `selection::row`, `activate`/`inspect` returning `Intent`). Tests, doctests, control page, module map, README.
3. Host adapter `tui::explorer::grid`: `CoreField` keeps the whole value and `cut_at` (`CoreField::long`); `ExplorerRow::NodeField` carries its field; edge roles mapped onto tones; unresolved stubs are inert link rows with edge meta under `E`; the hint names `keymap::INSPECT_KEY`.
4. `render_explorer_lines` and the Preview's field, edge, and evidence lines on `PropertyGrid` (the neighbor/unresolved headings stay headings); Enter's copy and `o`'s inspect read the row's `Intent`; clipboard sink and value modal stay in the host.
5. Nine typed `…` marks onto `marks::ELLIPSIS`; host guard refuses a string a bare mark leads.
6. Tests updated mechanically; adapter tests; placeholder copy test.
7. After frames, compare by category; ADR 0187; operator-review rows; design, changelog, operations; full checks; commits; final summary.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decisions while implementing: dropping edge meta when the label would fall under 8 cells hid `E` entirely in a 60-cell pane (two ui tests caught it), so meta is kept and wraps as before. Converting the Preview's `neighbor`/`unresolved` headings to property rows pushed a 34-cell name onto its own row at a 48-cell pane, so they stay headings. The unresolved stub's provenance/confidence became edge meta (shown with `E`) so the one-line row keeps its identity. Frames: 207 captured; 75 identical, 10 blank-only restyles, 66 Other-arrow color only, 56 text changes from the label gap and the unresolved rows. Full isolated suite: 4504 passed, 0 failed, 1 ignored. Filed CSP-788 (the unresolved row's Enter hint names `o`, which opens nothing there).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::property_grid` (ADR 0187): `PropertyRow` (a bold label in a fixed 14-cell column that keeps a blank cell before its value, a glyph, the value as text or spans, cut in cells at its end or middle with a `(truncated · KEY)` hint naming the host's inspect key, a placeholder that recedes, a hint, an annotation), `LinkRow` (indent, 22-cell verb column, glyph or the no-value mark, label cut in the middle to the room left, meta kept and wrapping past the width when the label would fall under 8 cells, and a generic `Tone` primary/quiet/alert/inert mapped to styles by `Tones`, the alert tone leading with `marks::WARNING`), `DisclosureRow` (the tree's `marks::disclosure` arrow in `disclosure`, a label, a count, tallies), and `PropertyGrid` (rows plus lines that are no row, every row through `selection::row`, `activate`/`inspect` returning `Intent::{Copy, Inspect, Toggle, Follow}`). Control page `crates/tuikit/docs/controls/property_grid.md`; module map and README rows.

Host: `tui::explorer::grid` maps the view model onto the rows (edge roles `edge_alt_of`/`edge_conflict` onto the quiet and alert tones); `CoreField` keeps the whole value and `cut_at` (the seven truncate-and-keep blocks became `CoreField::long`); `ExplorerRow::NodeField` carries its field; Enter's copy, `o`'s inspect, and the Preview subject read the row's `Intent`/field, while the clipboard sink and the value modal (`Pager`) stay in the host; the hint names `keymap::INSPECT_KEY`. The explorer zone and the Preview's field, edge, and evidence lines are grids; `FieldValue`, link semantics, verbs, and kind glyphs stay in the host. Nine typed `…` marks moved to `marks::ELLIPSIS`, and the host guard now refuses a string a bare mark leads. Decided: no `ColumnSchema` for the grid; `DisclosureRow` shares the tree's mark, not the left pane's row; the explorer state machine is untouched (CSP-755/756).

Verified: before/after `conspectus tui --snapshot` of the right pane (69 states, focused and unfocused, at 160x40, 100x30, 60x20; /tmp/tuifw-baseline/frames-754-{before,after}, summary in frames-754-summary.txt): 75 identical, 10 blank-cell restyles only (the Preview edge/evidence lead), 66 differ only in the Other arrow's color, 56 differ in text from the label gap and the unresolved rows. Tests: 22 framework grid tests and 5 doctests, 7 adapter tests, updated explorer/ui/app tests (mechanical: `long_value` became the whole `value` plus `cut_at`; renderer calls became grid rows; expectations unchanged), a new placeholder-copy test, a guard self-test for leading marks. Checks: fmt, clippy (workspace, all targets/features, -D warnings), the full isolated suite (4504 passed, 0 failed, 1 ignored) and nextest (4504 passed, 1 skipped), rustdoc -D warnings, `cargo build --examples`, the independence guard, `git diff --check`.

**Regressions**
- An unresolved row under Other no longer shows its provenance and confidence unless edge meta (`E`) is on, and its evidence is cut in the middle to one line (it wrapped onto 2-3 rows); the Preview still shows the evidence whole.
- A long unbroken value beside a label of 14+ cells (a path under `active_pane_current_path`) now starts on the row under its label.
- Enter on a placeholder field (`—`, `— (unknown)`) copies nothing and posts no toast (it copied the dash).

**Consistency wins**
- A label as wide as its column keeps a space before its value (`current_commit —`, `last_active_epoch 1790766000`; they ran together).
- Every related row, unresolved stubs included, is one line with the same layout and shows edge meta only under `E`.
- The Other header's arrow is the tree's (cyan `disclosure`, not gray bold), so an operator's `disclosure` reaches it.
- The cut hint names the key the keymap binds; copy, inspect, hint, and the drawn value come from one row.
- Glyphs come from the table: the unresolved row's `—` and nine `…` marks; frames byte-identical there.
- Highlight extent and the unfocused cursor were already the shared look (CSP-727): unchanged, verified in both focus states.
<!-- SECTION:FINAL_SUMMARY:END -->
