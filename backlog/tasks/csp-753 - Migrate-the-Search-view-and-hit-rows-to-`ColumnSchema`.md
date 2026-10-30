---
id: CSP-753
title: Migrate the Search view and hit rows to `ColumnSchema`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 05:25'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-752
  - CSP-743
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
priority: medium
type: enhancement
ordinal: 839000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A search hit renders in three separate column grids (the modal's entity and content grids and the left pane's hit, pane, and pane-hit rows), and quick select does not reuse the left-pane row renderer. Hit, pane, and pane-hit rows hard-clip at the pane edge, and the age cell is separate (inventory slices 02 and 05).

Scope: Render the Search view's session, hit, pane, and pane-hit rows from schemas that share the column plan used by the search modal, with the age cell and snippet cell from the framework. Keep the lazy hit children and the pane splice by capture time.

TUI changes:
- Consistency wins: search hits look the same in the modal and in the view; narrow-width behavior matches the other views
- Regressions or behavior changes: hit and pane rows stop hard-clipping and drop cells by priority instead

Tests and checks: The `consistency_tests/search_{flat,view}.rs` cases pass or are updated with a note. `--snapshot` of the Search view at 60, 80, and 110 columns.

Blockers: The previous migration and the search palette.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The Search view's row kinds render from schemas that share the modal's column plan
- [x] #2 Hit rows use the framework's age and snippet cells
- [x] #3 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #4 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #5 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
- [x] #6 The modal's content hits and the Search view's hit, pane, and pane-hit rows are one definition; the quick-select entity rows keep their own split and are tracked as a follow-up
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Framework (own commit, generic, tests, columns.md): `tuikit::columns` gains what the palette's `ColumnPlan` had and the schema lacked: a share of the row's whole width (`Column::share(key, per, of).within(min, max)`), steps by the row's whole width (`Column::row_steps`), and a fill's own minimum (`.at_least(n)`), all counting the row's lead (a cursor gutter, an indent).
2. Host: the hit's columns become one declaration, `hit_columns` in `src/tui/widgets/search/rows.rs` (source word 7, chip, label a fifth of the row 12..32, tag 22/20/14 by the row, age 4, snippet of at least 20; chip -> source -> age -> label narrows to 8), and one cell builder (`HitCells`), with the framework's age cell and snippet cell. The modal's content rows draw from it (text byte-identical; `content_columns` keeps its API, laid out by the schema); then `palette::ColumnPlan` is retired (framework commit).
3. Host: the Search view's hit, pane, and pane-hit rows draw from the same declaration after the tree's prefix (flat rows with chip and label; nested rows with tag, age, snippet), replacing `render_hit_spans`, `render_pane_spans`, `render_pane_hit_spans`, `append_content_hit`, `append_snippet`. Session rows keep the session schema shared by every view (ADR 0183). Lazy hit children and the pane splice untouched.
4. Entity rows: check whether the schema can express `entity_columns` cleanly; if not, leave them and record why.
5. Tests: framework unit tests; host narrow-width drop order for hit, pane, pane-hit rows; a hit lays out alike in the modal and the view; existing search tests unchanged.
6. Before/after `--snapshot` frames of the Search view and the modal at 160/110/80/60/40 x 30 (`/tmp/tuifw-baseline/frames-753-*`), compared by category.
7. Docs: `docs/tui-columns.md` (Search rows, narrow-width table; `just tui-columns`), `crates/tuikit/docs/controls/columns.md` and `palette.md`, a new ADR, `docs/design.md`, CHANGELOG, operator review.
8. Checks: fmt, clippy, isolated suite, rustdoc, examples, diff --check, independence guard.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in six commits: framework rules (share, row steps, fill minimum) 1b73951e; the modal on the schema e34e4d61; `palette::ColumnPlan` retired 3d1d0f89; the Search view's hit, pane, and pane-hit rows on the hit columns 50ee0ff7; ADR 0185 and docs be27f2d8, 1b3d812e.
Frames: `/tmp/tuifw-baseline/frames-753-before`, `-mid` (modal only moved), `-after`; 205 frames at 160/110/80/60/40 x 30: 92 identical, 17 modal frames restyled text only (text identical), 96 view frames with text changes, all in hit rows (687) and pane rows (50); 405 session, header, and search-header rows byte-identical with styles. Snapshot mode finishes a scan before the next key, so no frame shows a scan in progress; the per-message nested grouping is not reachable by keys (unit tests cover it).
Checks: fmt, clippy (workspace, all targets, all features, -D warnings), isolated full suite 4467 passed, 0 failed, 1 ignored (12 palette plan tests removed, 8 added), RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps, cargo build --examples, git diff --check, independence guard. Follow-up: CSP-786.

AC 2 originally said all three hit grids become one definition. The entity rows of quick select need two width rules that the shared column plan cannot express yet, so only two of the three grids were unified; the reworded criterion states that, and the third grid is filed as its own follow-up.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A search hit is one column schema in the contents modal and the Search view (ADR 0185; amends ADR 0175 decision 3 and ADR 0183).

Framework (generic, own commits, tests): `tuikit::columns` gained `Column::share(key, per, of).within(min, max)` (a part of the whole row), `Column::row_steps` (steps by the whole row's width whatever the columns before it take), and `.at_least(n)` (a fill's own minimum); both rules of the row count its lead (a cursor gutter, an indent). `palette::ColumnPlan`, `Column`, and `ColumnLayout` are retired: one narrow-width policy lays out every row of several cells. `crates/tuikit/docs/controls/columns.md` and `palette.md` updated; the share's rustdoc carries the palette page's example as a test.

Conspectus: `hit_columns(key, chip, HitShape)` in `src/tui/widgets/search/rows.rs` is ADR 0149 item 25 as one declaration (source word 7, chip or a pane's command, name a fifth of the row 12 to 32, tag 22/20/14 by the row, age 4, snippet of at least 20; the chip gives way first, then the source word, then the age, then the name narrows to 8), and `Hit::cells` one cell builder (the framework's age cell, tinted by recency; `snippet_cell`, which the left pane's session and mux rows use too; `hits_tag`, `turn_tag`, `line_tag`). The modal draws `Sourced` hits (`ContentGrid`, measured once a frame; `ContentRow::recency` added); the Search view's pane rows and Messages rows draw `Named` hits after the tree's prefix, and a turn or pane line under its parent `Nested` ones. `render_hit_spans`, `render_pane_spans`, `render_pane_hit_spans`, `append_content_hit`, `append_snippet`, `age_span`, and their width literals are gone. Session rows keep the session schema every view shares; quick select's entity rows keep `entity_columns` (the schema has no share of what the other columns leave and no fill that hands its room to a neighbor; recorded as a seam). Lazy hit children (`detail_pending`), the pane splice by capture time, and every Search view behavior are untouched (`rows/search.rs` unchanged).

Verification: `conspectus tui --snapshot` frames of the Search view (empty, Sessions and Messages listings flat and by project and harness, cursor on a row, unfocused, no match, pane hits) and the modal (content search by session and by message, moved, no match, pane search, quick select current and all, the modal over the view) at 160/110/80/60/40 x 30, before and after (`/tmp/tuifw-baseline/frames-753-before`, `-after`; `-mid` after the modal alone moved): 205 frames, 92 identical; 17 modal frames restyled only, text identical at every width; 96 view frames changed text, every change in a hit row (687) or pane row (50); 405 session, header, and search-header rows byte-identical with styles. Snapshot mode finishes a scan before the next key, so no frame shows one in progress, and the nested per-message grouping is not reachable by keys (unit tests cover it). New tests: framework unit, property, and doc tests for the rules; the drop order and fit of hit, pane, and pane-hit rows at every width; a hit that reads as the modal's row at one width; the Messages listing's turn column and marked cuts at 60/80/110/160. Two checks changed with a note: `search_flat::the_columns_line_up_down_the_list` counted the turn column in bytes (a name cut with `…` moved it) and now counts characters, expectation unchanged; the showcase screenshot finds the Search view's pane row by its glyph, name, and count, since its command gives way first and the 73-cell pane is a cell short. Content test fixtures gained the `recency` field. `just tui-columns` regenerated six Search frames; prose updated. Checks: fmt, clippy (workspace, all targets, all features, -D warnings), the isolated full suite (4467 passed, 0 failed, 1 ignored; 12 plan tests removed, 8 added), nextest, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `cargo build --examples`, `git diff --check`, the independence guard. Follow-up: CSP-786.

**Regressions**
- A matching turn no longer shows who spoke (`you`, `ai`, `system`): its tag is the modal's `turn N · kind`.
- At 160 columns a hit's tag is 14 cells and cuts its kind (`turn 46 · mes…`): an overflowing list's rows are 77 cells, under the 78-cell step; the old row showed `turn 46    ai      message` whole (CSP-786).
- A pane row's command gives way first: it is gone from the Sessions listing's pane row at 160 and 80 (prefix with pin marker), and at 110 and 60 the age goes too, to keep a 20-cell snippet; hit rows likewise lose their chip at 110, 60, and 40 and their age at 110 and 40.
- The view's flat hit names and pane names are no longer bold; `turn N` and `line N` are dim where they were gray.
- Frames in `docs/tui-columns.md` change (six Search view frames).
- The gaps between a modal row's cells lose their dim style (visible only inside the cursor's highlight).

**Consistency wins**
- A search hit reads the same in the modal and in the Search view at one width: one column declaration, one cell builder, the same widths and drop order; one column engine in the framework.
- Hit, pane, and pane-hit rows never stop at the pane's edge: cells give way by priority and cuts end in `…` (at 110 a row stopped at `messag`).
- Snippets show in narrow panes: at 130 and 110 columns a session's hit rows show their snippets (they showed none), and the Sessions listing's pane rows at 110, 60, and 40 (they showed none).
- A pane row reads as the modal's pane result: its command, its mux and place as one name, its hit count, its age, its snippet.
- The modal's ages are tinted by recency like every list's, and its snippets take the lists' snippet look.
- Session rows, group headers, the Search header, the empty states, and quick select (both reaches) are unchanged, styles included.
<!-- SECTION:FINAL_SUMMARY:END -->
