---
id: CSP-751
title: Add `ColumnSchema` and migrate the Sessions and Mux views
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 02:32'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-750
  - CSP-723
  - CSP-725
  - CSP-721
  - CSP-711
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/tui-columns.md
priority: high
type: enhancement
ordinal: 837000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
There is no declarative column description in the TUI: seven `render_*_spans` functions plus inline group-header and fork code make about ten layouts, three alignment structs (`GroupAlign`, `PrAlign`, `MixedAlign`), about 25 width literals, and about 40 hard-coded two-space gutters. Narrow-width behavior is not one policy: Mux rows drop cells and end in an ellipsis, Session rows hard-clip at the pane edge, and the "room remains" cutoff takes three values (above 2, 4, and 8). Ghost muxes emulate the grid with `MixedGrid::Mux` (inventory slice 02, sections 4 and 6).

Scope: Create `ColumnSchema { columns: [Column { key, width: Fixed | Cap(n) | Fill, align, drop_below, priority }] }` and typed cells (text, chip, age, status glyph, right-aligned count, muted secondary with middle ellipsis, snippet), with one measure pass replacing the alignment structs and one narrow-width policy (drop by priority, end in an ellipsis). Migrate the Sessions and Mux views. Regenerate the frames in `docs/tui-columns.md` with `just tui-columns` and edit the prose by hand, as the project requires.

TUI changes:
- Consistency wins: one narrow-width policy for Sessions and Mux; the ghost-mux grid emulation disappears; a test that columns line up comes with the schema
- Regressions or behavior changes: which cells drop first at narrow widths changes for Sessions and Mux rows; frames in `docs/tui-columns.md` change

Tests and checks: The `tui_columns_doc` tests, `consistency_tests/{rendering,docs}.rs`, and the rows and `ui_tests.rs` cases pass or are updated with a note. `--snapshot` at widths 60, 80, 110, and 160.

Blockers: The tree extraction, text, chips, and theme.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `ColumnSchema` and the typed cells exist with one measure pass and one narrow-width policy
- [x] #2 The Sessions and Mux views render from schemas and `GroupAlign`, `MixedAlign`, and `MixedGrid::Mux` are removed
- [x] #3 `docs/tui-columns.md` prose and frames match the new behavior and its doc-sync tests pass
- [x] #4 A cross-view test asserts that columns line up at several widths
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
1. Framework `tuikit::columns` (new module): `ColumnSchema<K>` of `Column<K>` (key; width `Fixed(n)`, `Cap(n)` measured over the tree, `Steps` by the room, `Hug(n)` (a cell as wide as its own text, what follows flows), `Fill`; align left/right; gap after; `drops(priority)`, `shrinks(priority, floor)`, `drop_below(room)`, `ragged()` for a column only the cells something follows widen). Typed `Cell`: text cut at the end, in the middle, or clipped, with a kept tail; spans and chips; a chip list with `+N`; the age cell; a snippet with its match painted; empty. `Measure<K>`: one pass over every row of the tree (folded rows included): the widest cell per key, the widest from the row's start, whether any row has one.
2. One narrow-width policy, `ColumnSchema::draw`: the columns that line up give way per room from the tree-wide widths (so every row loses the same ones); the cells after the first hugging cell give way row by row; in both, columns give way in ascending priority (drop, or shrink to a floor) until the row fits; the fill takes what is left, shown only with at least `MIN_FILL` (8) cells, cut with `…`; a row still too wide is cut at its edge with `…`. Unit and property tests in the crate.
3. `tuikit::tree`: `RowColumns<R>` (the host's `schema(&R)` and `cells(&R, &RowCtx) -> Cells<K>`) and `TreeColumns`, which measures every row once per frame and lays each window row out after its indent for the `TreeView` producer; `line_then` for rows whose rest the host still draws; a disclosure cell helper.
4. Host `ui/left_panel/columns.rs`: the left pane's keys and schemas: the prefix (disclosure, kind glyph, header glyph, pin marker), the group header (label, detail cut in the middle with `(cwd)` kept, count, warning), the session row, the mux row (stepped name, a chip list capped at two chips, age, state, preview), shared by live and stopped muxes. Producer `LeftColumns: RowColumns<Row>`.
5. Migrate in slices, each green: (a) headers and session rows (all views they appear in); (b) mux rows and stopped muxes on the mux schema (whole-row ghost look; `MixedGrid` deleted); PR, Pinned/History, hit, and pane rows keep their bodies via `line_then`, with the Pinned/History widths from the one measure pass (`MixedAlign` replaced; `PrAlign` stays for CSP-752); (c) delete `GroupAlign`, `MixedAlign`, `RowPrefix`/`row_prefix_spans`, and the span renderers and width tiers they replace.
6. Tests: update the ui_tests, rows, and consistency cases with notes; a cross-view consistency test (testkit token columns) that columns line up at 60, 80, 110, 160 in Sessions and Mux (all groupings, stopped muxes) and that a session row lays out alike in Sessions, Mux, and PRs.
7. Docs: `crates/tuikit/docs/controls/columns.md`, the README and architecture module map, tree_view seams; ADR 0183 (schema, policy, thresholds, measure scope, the CLI's `fit_to_width` stays separate, stopped-mux look); `docs/tui-columns.md` prose (narrow-width policy) and `just tui-columns` frames; operator-review rows.
8. Verify: before/after `--snapshot` frames at 160/110/80/60/40 by 30 in `/tmp/tuifw-baseline/frames-751-*`, every difference categorized; fmt, clippy, the isolated suite, rustdoc with `-D warnings`, the examples build, and the whitespace check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Slices: (1) `tuikit::columns` (schema, typed cells, `Measure`, one narrow-width policy) and `tree::{RowColumns, TreeColumns}` with unit and property tests and `docs/controls/columns.md`; (2) group headers and session rows on schemas in every view, `GroupAlign`/`MixedAlign`/`RowPrefix` and the header and session span renderers removed, Pinned/History widths from the one measure pass; (3) mux rows and stopped muxes on one mux schema, `MixedGrid` and the mux span renderers removed, stopped muxes whole-row ghosts; (4) a cross-view consistency test (token columns at 60/80/110/160) and narrow drop-order tests; (5) ADR 0183, design, operator review, changelog.

Found while migrating: today's header counts line up across depths (the old body width counted the indent) while the label drifts (CSP-772); row-relative measuring moved nested counts, so the framework gained `absorbs_lead` and the header's path uses it. CSP-772's label drift is left as it was (one flag for CSP-752).

Thresholds chosen against the before/after frames: `MIN_FILL` 7 reproduces the old preview rule (8 dropped 7-cell previews at 80 columns); a row's last cell needs only its own text (a flat project whose pad overflowed was dropped); a shrink floor gives way to a shorter whole text (a 7-cell project fits whole).

Frames: `/tmp/tuifw-baseline/frames-751-before` and `-after` (1095 left-pane frames each, every view and grouping, folded and unfolded, cursor on a header and a row, focus on and off, stopped muxes shown, content filter, at 160/110/80/60/40 by 30), compared with `scripts751/compare.py` and `styled_cells.py`.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::columns`: `ColumnSchema<K>` of keyed `Column<K>`s (width `Fixed(n)`, `Cap(n)` measured over the tree, `Steps` by the room, `Hug(n)` for a cell that keeps its own width, `Fill`; `Align::Left`/`Right`; a gap after, `GAP` 2; `drops(p)`, `shrinks(p, floor)`, `drop_below(room)`, `ragged()`, `absorbs_lead()`), typed `Cell`s (text cut at the end, in the middle, or clipped, with a kept tail; spans and chips; a chip list with `+N`; the age cell; a snippet with its match painted), `Measure` (one pass over every row of the tree, folded rows included; shared keys share a column), and one narrow-width policy in `ColumnSchema::draw`: columns give way in ascending priority until the row fits (the lined-up ones by the whole tree's widths, so every row at a width loses the same ones; the cells after the first hugging one row by row), the fill takes the rest and shows only with `MIN_FILL` (7) cells or its whole text, a narrowing cell stops at its floor (8 for labels, paths, and projects; 10 for mux names), a row's last cell needs only its own text, a ragged cell that ends its row runs on into the room, and a row still too wide is cut at its edge with `…`. `tree::RowColumns<R>` (the host's `schema(&R)` and `cells(&R, &RowCtx) -> Cells<K>`) and `TreeColumns` (measures every row, lays each out after its indent; `line_then` for kinds not on a schema yet) feed `TreeView`. Reference page `crates/tuikit/docs/controls/columns.md`; ADR 0183.

Conspectus: `ui/left_panel/columns.rs` declares the prefix, group header, session row, and mux row schemas and their cells (tree_label disambiguation, keys, pin markers, `was in`, hit counts, previews and snippets, header counts and warnings unchanged). Headers and session rows use them in every view; mux rows and stopped muxes share the mux schema. `GroupAlign`, `MixedAlign`, `MixedGrid` (and the `grid` field), `RowPrefix`, `row_prefix_spans`, the session, header, and mux span renderers, `mux_label_column_width`, and their width and gutter literals are gone; the Pinned and History rows read their widths from the one measure pass, and the PR, Pinned, History, hit, and pane rows keep their own cells after the framework's prefix until CSP-752 and CSP-753 (`PrAlign` stays). The CLI's `fit_to_width` stays separate (ADR 0183 decision 7). CSP-772 is not fixed: header counts still line up at every depth (the path absorbs the indent), and the label drift is one flag (noted on CSP-772). CSP-775's pointer gutter is not added.

Verification: 1095 `--snapshot` left-pane frames with ANSI before and after (`/tmp/tuifw-baseline/frames-751-before`, `-after`: every view and grouping, folded and unfolded, cursor on a header and a row, focus on and off, every stopped mux, the content filter, at 160/110/80/60/40 by 30), compared cell by cell. Pinned and History: no text change (blank-cell styles only). At 160 and 80 the only text change is the flat grouping's cut project (`showcase-deck-…`, was clipped `showcase-deck-c`); at 110 labels at the pane's edge end in `…` (were clipped), the flat project and hit count follow the drop order, a 5-cell project fragment leaves, and a stopped mux's 4-cell `~/r…` leaves (under 7 cells); 60 and 40 follow the policy (keys, then chips, then ages leave session rows; mux names narrow before ages leave). Styles: only the stopped mux's `▣` (ghost look) among text cells; blank cells lose gutter styles. Tests: framework unit, property (300 random schemas: no row wider than its room; lined-up cells in one place), and doc tests; host `consistency_tests/columns.rs` (token columns at 60/80/110/160: ages, states, mux names, and header counts line up in every Sessions and Mux grouping; a session lays out alike in Sessions and Mux; a mux whose chips overflow keeps its age in line); narrow drop-order tests for session, mux, and header rows; ui_tests now lay rows out through the real pipeline; badge tests (the notes lost their built-in gap). `just tui-columns` regenerated one frame; the prose gains "When a row is too narrow" and the Mux and Session table updates. fmt, clippy (workspace, all targets, all features, -D warnings), the isolated full suite (4462 passed, 0 failed, 1 ignored; an earlier run hit the known "Text file busy" and `discovery::forge` timing flakes and was green on rerun), `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `cargo build --examples`, and `git diff --check` pass.

**Regressions**
- At narrow widths a session row drops its key first, then its chip, then its age (it was cut at the pane's edge, keeping the key); a mux row narrows its name with `…` and drops its age before its chips (it kept the whole name and the chip and lost the age and state).
- A stopped mux's directory needs 7 cells (it showed from 3, e.g. `~/r…` at 110 columns).
- A label, project, or path that cannot keep 8 cells leaves the row instead of showing a fragment.
- In content-filtered and Search session rows the hit count outlasts the flat project when both do not fit.
- Frames in `docs/tui-columns.md` change (one row).
- The gaps between cells lose styles some carried (visible only inside the focused cursor's highlight).

**Consistency wins**
- One narrow-width policy for group headers, session rows, and mux rows, in every view that lists them; no cell is cut at the pane's edge without `…`; one fill threshold replaces the >2, >4, and >8 cutoffs for them.
- The ghost-mux grid emulation is gone: a stopped mux is drawn from the mux schema and wears the ghost look whole (its `▣` included), like every gone row of a single-kind view.
- A mux with several agents keeps its age and state in line with the others (the chips column is as wide as the widest row's).
- A narrow header keeps its count and warning (the warning was clipped) and narrows or drops its path.
- A test asserts that columns line up at 60, 80, 110, and 160, and that a session row lays out alike across views.
<!-- SECTION:FINAL_SUMMARY:END -->
