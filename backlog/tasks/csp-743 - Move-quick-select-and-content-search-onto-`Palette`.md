---
id: CSP-743
title: Move quick select and content search onto `Palette`
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-29 14:20'
labels:
  - tui-framework
  - tuifw-choice
milestone: m-25
dependencies:
  - CSP-742
  - CSP-723
documentation:
  - docs/plans/tui-framework/inventory/05-search-help-viewers.md
  - docs/adr/0149-two-level-search-current-and-wider-context.md
priority: high
type: enhancement
ordinal: 829000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`widgets/search.rs` is 1,523 lines holding two modes: quick select (`/`) with a Current and All scope, and content search with an options pane, a debounce and progress state machine, and history paging. Search results appear in three separate hit-column grids and quick select does not reuse the left-pane row renderer. The hit chip width is hard-coded at 10 (inventory slice 05, sections 2.9 and 6).

Scope: Rebuild both modes as `Palette` instances with Conspectus row types and a shared `ColumnPlan` for the hit columns (the same plan the Search view uses later). The ranking, the entity index, outcome words, content kinds, harness badges, and history paging stay in Conspectus. Keep ADR 0147 and ADR 0149 semantics exactly: the search behavior does not change.

TUI changes:
- Consistency wins: one hit-column plan instead of three grids; the hit chip honors `theme.badge_width`
- Regressions or behavior changes: none expected

Tests and checks: The existing `search_quick_tests.rs`, `search_content_tests.rs`, `consistency_tests/search_*.rs`, and `tests/cli_search.rs` pass unchanged. `--snapshot` of both modes at 80x24 and 40x10.

Blockers: `Palette`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Quick select and content search are `Palette` instances and `widgets/search.rs` shrinks to the adapter
- [x] #2 A shared column plan drives the modal's hit rows
- [x] #3 Search behavior is unchanged: every search test passes without edits to expectations
- [x] #4 The hit chip width comes from the theme
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
1. Capture before frames of quick select (Current, All, empty, filtered, no match, a 30-row list moved down and back up, typed after moving) and content search (empty, results with snippets, Messages, options pane, a toggled kind, pane hits, no match, moved, Current, recalled) at 100x30, 60x20, 40x10.
2. Framework (own commit): add to `tuikit::palette` a generic `ColumnPlan` (ordered columns: fixed, a bounded share, steps, one fill; gaps; drop and shrink give-way order; start/end alignment; a lead) laid out by `ColumnLayout`; `Progress` (a provider's status words); `Palette::draw_with` and `handle_key` (rows the host draws, which need not be `PaletteRow`s); `Palette::select` (the host moves the cursor); `Palette::reserve_scrollbar` (rows keep one width as a list overflows). Tests and palette.md.
3. Lay the content hits out from the hit plan on the old widget first, byte for byte.
4. Move both modes onto `Palette` instances over `SearchKey` (`PaletteAction`): quick select with the Current/All strip and host-ranked rows, outcome words through `draw_with`, and its own cursor rule kept with `select`; content search with both strips, the options pane, the debounce, `Progress` in the status row, and the verbs (regroup, promote, history paging) as host actions. Row layouts move to `search/rows.rs`; test helpers change mechanically only.
5. Retire `SEARCH_OPTION_KEYS`/`SearchOptionKey` (the palette's `PALETTE_OPTION_KEYS` is identical).
6. Docs: ADR 0175, design.md, operations.md, CHANGELOG, operator-review rows; after frames and a diff; fmt, clippy, the isolated suite, nextest, rustdoc, examples, diff-check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Framework: `ColumnPlan`/`Column`/`ColumnLayout`, `Progress`, `Palette::draw_with`, `handle_key`, `select`, `reserve_scrollbar` (16b99e50). Host: the hit plan on the old widget first, byte-identical (386dc33d); then both modes on `Palette` with row layouts in `search/rows.rs` (5a90dcff); `SEARCH_OPTION_KEYS` retired (c5063756); ADR 0175 and docs (b037f0fa). Before/after frames under `/tmp/tuifw-baseline/frames-743-*`: 54 of 69 byte-identical with styles, 15 overflowing quick-select lists differ by the scrollbar and the kept window only. Filed CSP-782 (PgUp/PgDn in the search tables).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed in `tuikit::palette`: `ColumnPlan`, `Column`, and `ColumnLayout` (ordered columns: fixed, a bounded share of the row, steps by the row's width, or the one fill; a gap after each; the order the others give way in, dropping out or narrowing to a floor, while the fill is short of its minimum; end alignment; a `lead` for the cursor gutter), `Progress` (a provider's status words), `Palette::draw_with` and `handle_key` (rows the host draws, which need not be `PaletteRow`s), `select` (the host moves the cursor), and `reserve_scrollbar`; `crates/tuikit/docs/controls/palette.md` covers them.

Quick select and content search are `Palette` instances over `SearchKey` (`PaletteAction`): `SearchOverlayState` holds quick select's (Current/All strip, count, results the host ranks with `rank` and `EntityIndex::rank`) or content search's (both strips, the options pane, the debounce, the status row with `Progress`). The adapter keeps the ranking, outcome words, content kinds, history paging, regrouping, the promote verbs, and the `Msg` mapping, and draws the rows (the host theme's glyph overrides and harness colors; outcome words only for rows shown) from `src/tui/widgets/search/rows.rs`. A content hit is laid out by `hit_plan` (ADR 0149 item 25 as a plan; the chip column is `chip::width(theme)`), which the Search view's rows can adopt (CSP-753). Quick select keeps its own cursor rule across an edit. `SEARCH_OPTION_KEYS`/`SearchOptionKey` are gone (`PALETTE_OPTION_KEYS` is identical). `widgets/search.rs` went from 1361 lines to a 967-line adapter plus 369 lines of rows. ADR 0175 (amends ADR 0174); design, operations, changelog, operator review updated.

**Regressions**
- None. Search behavior is unchanged: every search test passes with its expectations unchanged.

**Consistency wins**
- A quick select or content search list taller than the modal shows the shared scrollbar, in the column its rows already left blank, so the rows keep one width and a content scan's columns stay put as results arrive.
- The list keeps its scroll window between frames: moving back up a scrolled list no longer drags it to the top.
- One column plan drives the content hits, with the chip width from the theme.
- The options pane takes the palette's standard option keys (same keys, same footer).

Verified with `conspectus tui --snapshot` frames of both modes at 100x30, 60x20, and 40x10 (`/tmp/tuifw-baseline/frames-743-*`, 23 states each): 54 of 69 byte-identical with styles; 12 overflowing quick select lists differ only by the scrollbar column and 3 moved-down-then-up lists by the kept window. Snapshot mode finishes a scan before the next key, so the in-progress status is covered by the existing unit snapshot. Test changes are mechanical only (drawing borrows the state mutably, `build_match_line` and `reach_segments` became test helpers, the row layouts are imported from `search::rows`, `overlay_tests.rs` names `PALETTE_OPTION_KEYS`); one test was added. Checks: fmt, clippy `--workspace --all-targets --all-features -D warnings`, the isolated suite (4270 passed, 0 failed, 1 ignored), nextest (4270 passed), rustdoc `-D warnings`, `cargo build --examples`, `git diff --check`, the independence guard. Follow-up: CSP-782 (PgUp/PgDn in the search tables).
<!-- SECTION:FINAL_SUMMARY:END -->
