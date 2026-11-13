---
id: CSP-752
title: 'Migrate the PRs, Pinned, and History views to `ColumnSchema`'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 04:12'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-751
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/tui-columns.md
priority: medium
type: enhancement
ordinal: 838000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The PRs view carries a tree-wide label width that shrinks to `#N` through a post-hoc rewrite, and its PR state-to-glyph logic exists in six places. Pinned is the only view with status words and drops the chip first at narrow widths. Group headers pack the label and the secondary text into one `display_path` string split on two spaces. History rows are all ghosts (inventory slice 02, section 5).

Scope: Migrate PRs, Pinned, and History to schemas using the one narrow-width policy. Replace the `display_path` packing with structured cells. Use the single PR state function from the left-pane defect task. Regenerate the frames in `docs/tui-columns.md`.

TUI changes:
- Consistency wins: one narrow-width policy across the first five views; group headers carry structured cells
- Regressions or behavior changes: which cells drop first at narrow widths changes for PRs, Pinned, and History; frames in `docs/tui-columns.md` change

Tests and checks: The Pinned snapshots at 70, 110, and 160 columns are regenerated and reviewed. The rows tests and doc-sync tests pass.

Blockers: The first schema migration.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 PRs, Pinned, and History render from schemas and `PrAlign` is removed
- [x] #2 Group headers use structured cells and `display_path` no longer packs two values into one string
- [x] #3 `docs/tui-columns.md` and its doc-sync tests are updated
- [x] #4 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #5 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #6 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. tuikit (own commit): a capped column caps each cell in the measure, so a lead-absorbing capped column ends in one place; a look over part of a row (`ColumnSchema::draw_with_look`, `TreeColumns::line_with_look`) for a row that dims its body and keeps its marks' colors. Unit tests and the columns reference page.
2. PRs: a PR schema (label capped over the tree, its `#N` kept whole as a tail while the forge repo narrows; tracking; age; state; then branch hugging 24 and title filling). The builder decides while filing PRs whether a row names its forge repo (no post-hoc rewrite). `PrAlign` and `render_pr_spans` go; PR tests move to the real pipeline; a narrow drop-order test.
3. Pinned and History: one mixed schema (label capped at 28 and lead-absorbing, status capped at 9, age, chip, content fill; the chip leaves first, then the content, then the age, then the label narrows). Dimmed rows wear the ghost look from the label on, so the kind glyph keeps its color. `MixedWidths` and `render_mixed_spans` go. Pinned 70/110/160 snapshots regenerated deliberately.
4. Structured group headers: `GroupRow { label, detail }` with constructors; every builder (Sessions, Mux, PRs, Pinned, History, Search) fills them; `compact_path_label`/`_secondary` replaced; quick select reads the header's text.
5. CSP-772: the header label absorbs the indent, the path is measured plainly, so nested headers' paths and counts line up; regression test with headers at two depths; extend the columns consistency test to PRs, Pinned, and History and header paths.
6. Docs: docs/tui-columns.md prose and frames, crates/tuikit/docs/controls/columns.md, ADR 0184, design.md if touched, CHANGELOG, operator-review rows. Before/after --snapshot frames (frames-752-before/after) compared and classified. Close CSP-772 and CSP-752.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Slices: (1) tuikit per-cell caps and `draw_split`/`line_with_look`; (2) PR schema; (3) tuikit depth-alike give-way, found while testing the Pinned drop order (a deeper row hit its label floor first and dropped its age alone); (4) Pinned/History schema; (5) structured headers; (6) CSP-772; (7) columns consistency test for PRs, Pinned, History, and header paths; (8) ADR 0184 and docs. Judgment calls are in docs/plans/tui-framework/operator-review.md (CSP-752 rows). Frames: /tmp/tuifw-baseline/frames-752-before and -after, compared with scripts752/compare.py; blank-style classes with scripts752/blank_styles.py.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The PRs, Pinned, and History views' rows are laid out from column schemas (`src/tui/ui/left_panel/columns.rs`), group headers carry two cells, and nested headers line up with their parents (CSP-772, closed here). ADR 0184.

PRs: the label (as wide as the widest in the tree), tracking mark, age, and state glyph line up; the branch (hug 24) and the title (fill) follow. The label is the forge repo (`PrRow.forge_repo`, none under a group that names it) and the number as the cell's tail, so a narrowing label cuts `owner/repo` and keeps `#N` whole. `build_prs_tree` files every PR under its bucket first and builds rows and headers knowing which groups name their repo; the post-hoc `#N` rewrite, `PrAlign`, and `render_pr_spans` are gone. Pinned and History: one schema, every cell lined up (label cut at 28 and ending in one column at every depth, state word capped at 9, age, chip, content); a dimmed row wears the ghost look from its label on through `TreeColumns::line_with_look`, so its glyph keeps its color; the hand-off spinner is padded to the word. `MixedWidths`, `mixed_widths`, `mixed_measures`, and `render_mixed_spans` are gone. Headers: `GroupRow { label, detail }` with `at_path`, `named`, `with_detail`, `text`; `workspace_header` and `workspace_header_detail` replace `format_workspace_display` and `workspace_header_label`; `compact_path_label` and `compact_path_secondary` are gone; every builder (Sessions, Mux, PRs, Pinned, History, Search) fills the two fields; quick select reads `text()`. CSP-772: the header label absorbs the indent and the path is measured plainly.

Framework (generic, own commits, tests): a capped column caps each cell in the measure and when drawn; `ColumnSchema::draw_split` and `TreeColumns::line_with_look`; a row whose lined-up columns include a lead-absorbing one gives way as a row with no lead would in the whole width (rows at every depth drop the same columns). `crates/tuikit/docs/controls/columns.md` updated.

Verification: 1095 `--snapshot` left-pane frames with ANSI, every view and grouping (PRs checkout/repo/state, Pinned project/kind/none, History), folded and unfolded, cursor on a header and a row, focus on and off, at 160/110/80/60/40 by 30, before and after (`/tmp/tuifw-baseline/frames-752-before`, `-after`): 832 identical, 135 restyled blanks only (PR and Pinned gaps), 0 restyled text, 128 text changed (all listed below). New tests: PR layout through the real pipeline and a narrow drop order; Pinned narrow drop order with state words in line at every depth; dimmed Pinned row keeps its glyph color; nested headers at three depths (fails with the old flags); the columns consistency test now draws PRs, Pinned, and History and checks header paths at every depth; framework unit and property tests. The Pinned 70/110/160 insta snapshots were rerun and reviewed: unchanged; `history_view_flat` and `slot_row_menu` were regenerated deliberately (fill minimum). `just tui-columns` regenerated two new frames (checkout grouping with nested headers; Pinned at 40 columns); prose updated. Checks: fmt, clippy (workspace, all targets, all features, -D warnings), the isolated full suite (4471 passed, 0 failed, 1 ignored, first run), `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `cargo build --examples`, `git diff --check`, the independence guard.

**Regressions**
- Narrow PR rows: a title shows only with 7 cells (it showed from 1; e.g. `Pin t…` and `R…` leave at 110 and 60); then the branch narrows and leaves.
- Narrow Pinned and History rows: the content needs 7 cells (2-5 cell fragments like `~/…`, `co…`, `proj…` leave at 60 and 40), and at 110 History drops its chip for content (`~/repos/project` instead of `claude` and `~/re…`).
- Quick select lists a header as its row reads it (`repo-a  ~/atelier-demo/repo-a`), not the bare path.
- The gaps of PR and live Pinned rows lose the colors they carried (visible only inside the focused cursor's highlight).
- A deeper Pinned row's label may narrow below 8 cells to keep its state word in line.
- Frames in `docs/tui-columns.md` change: two frames added; existing frames unchanged.

**Consistency wins**
- One narrow-width policy for every left-pane row but the Search hit and pane rows: a narrow PR row keeps its number and state glyph (both were cut off at the pane's edge), and a narrow Pinned or History row keeps its name and state word and age (all three were cut off at 40 columns).
- Nested group headers line up with their parents: paths start in one column at every depth (they drifted 2 cells per level), and counts stay in line when the path gives way.
- Group headers carry structured cells; nothing splits a header string on two spaces.
- PR and Pinned/History widths come from the one measure pass; no host pre-pass is left.
<!-- SECTION:FINAL_SUMMARY:END -->
