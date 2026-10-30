---
id: CSP-750
title: Extract `TreeState<Id>` and `TreeView<R>` with app-rendered lines
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 00:02'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-724
  - CSP-727
  - CSP-725
  - CSP-723
  - CSP-714
documentation:
  - docs/plans/tui-framework/inventory/02-left-pane-tree.md
  - docs/adr/0151-tui-framework-control-set-and-house-rules.md
priority: high
type: enhancement
ordinal: 836000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The left pane is not a widget. `draw_left_panel(&mut App, ...)` reads about 20 `App` accessors, builds one `Line` per visible row, and hands the lot to a `Paragraph`. Tree state is five loose fields on `App` mutated by `app/tree.rs` and reducer arms, selection retention is duplicated between `set_data` and `set_tree`, three expand and collapse scans repeat, and the draw dispatches `Msg::LeftViewportChanged` mid-render. `RowKind` and `RowId` are also read by header counts, preview selection, the quick-select index, the keymap, and actions, so a generic control cannot own them (inventory slice 02, sections 0 to 3).

Scope: Create `TreeState<Id>` (selection retained by id with the nearest-duplicate tiebreak, expansion set, scroll, measured viewport, paging, home and end) and `TreeView<R>` (pane chrome slots for the tab title, header line, footer banner, and empty state, the body, and the scrollbar gutter), with rows generic over `Id` and cell content supplied by the app as a `Fn(&R, RowCtx) -> Line`. This is the stepping stone: the opinionated column layout stays in app code until the column schema stories. The renderer reports the measured viewport itself, so the mid-draw message goes away. Domain lookups in the renderer (`pinned_nodes()`, `presence()`, `row_is_frozen`) move to the app's row producer.

TUI changes:
- Consistency wins: PageUp and PageDown move by the measured viewport; no blank tail past the content; one retention routine
- Regressions or behavior changes: paging distance changes (was terminal height minus 2)

Tests and checks: The `rows` tests, `ui_tests.rs` scroll tests, and `consistency_tests/{state,rendering}.rs` pass. Property tests for selection retention across rebuilds. `--snapshot` of the Sessions, Mux, and Pinned views at 80x24 and 40x10.

Blockers: Scroll, selection, chips, and text, plus the left-pane defect task.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `TreeState<Id>` owns selection, expansion, scroll, viewport, and paging, and the five loose `App` fields and the duplicated retention and expand/collapse code are removed
- [x] #2 `TreeView<R>` renders the pane chrome slots, the body from app-supplied lines, and the scrollbar gutter
- [x] #3 No renderer dispatches a message during `draw`
- [x] #4 `RowKind` and `RowId` stay in Conspectus and the framework control is generic over `Id`
- [x] #5 Domain lookups no longer run inside the renderer
- [x] #6 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
- [x] #7 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #8 The control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames (every view, expand/collapse/paging/regroup/view-switch keys, focused and unfocused) at 100x30, 60x20, 40x10 into /tmp/tuifw-baseline/frames-750-before.
2. Framework slice: `tuikit::tree` with the `TreeRow` trait (id, depth, expandable) and a plain `Row<Id, R>`; `TreeState<Id>` (selection by id with the kept position as the duplicate tiebreak, expansion set, kept window, step/page/home/end over the measured viewport, expand/collapse/toggle/reveal, visibility by collapsed ancestors, one rebuild routine with the clamp and first-load hint fallbacks, `set_viewport`); `TreeView` (title, header line, footer banner, empty text, body from a host `Fn(&R, &RowCtx) -> Line` producer, the pending/ghost looks and the highlight, the scrollbar gutter) returning the window it drew; unit and hand-rolled property tests.
3. Host state slice: `impl TreeRow for rows::Row`; `App` holds one `TreeState<RowId>` (also in `ViewStateSlot`) instead of `expanded`, `selection`, `left_window`, `last_visible_index`; `set_data`/`set_tree` share the retention routine; expand/collapse/toggle and the two ancestor walks go through the state; page keys page by the kept window (the `Msg` payload and `translate`'s height argument go).
4. Renderer slice: `draw_left_panel` builds a `TreeView`; the row producer takes a `RowCtx` (expanded, cursor, mark, pending, width) and no longer reads the snapshot; the App precomputes each row's ghost, pending, and mark before the frame.
5. Docs slice: control page `docs/controls/tree_view.md`, architecture and README rows, ADR 0182 (amends 0031, 0156), ADR index, design.md, operator-review rows.
6. After frames, byte-compare; full checks (fmt, clippy, isolated suite, doc, examples, diff --check); finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Slices: d22d7104 framework control (tuikit::tree with unit and property tests); db8870fc host state (TreeState<RowId> on App and in ViewStateSlot, one rebuild routine, folds and ancestor walks through the state, page keys without a payload); 204b8e75 renderer (TreeView in draw_left_panel, LeftRows producer, App::tree_row_states precompute, source check); 01cb13ab tests (window follows the cursor's copy, FRAMEWORK_SRC tripwire 126 of 168); 7ed3c402 framework docs; 4b77ece6 ADR 0182; b3f8f150 design, changelog, operator review.

Decisions: rows enter through a `TreeRow` trait on the host's own `rows::Row` (no `kind` -> `payload` rename); retention uses the copy the cursor stood on (kept position), which changes behavior only for duplicated rows; row states are a per-frame slice, not a cache or a draw-time closure; mixed rows keep their body-only ghost; the tree has no key table yet (seam). All recorded in operator-review.md and ADR 0182.

Frames: 771 before/after `--snapshot` frames (ANSI) across Sessions, Mux, PRs, Pinned, History, Search at 100x30, 60x20, 40x10 are byte-identical; capture scripts in /tmp/tuifw-baseline/scripts750/.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::tree`: `TreeRow` (identity, depth, children) with a plain `Row<Id, R>`; `TreeState<Id>` (selection by id with the position it showed at as the duplicate tiebreak, the expanded set, the window the last frame drew, step/page/home/end clamped at the ends and paging by the kept window, expand/collapse/toggle/reveal, visibility by collapsed ancestors, one rebuild routine `begin_rebuild`/`finish_rebuild`: same id, else same position clamped, else first-load hint, else first row, `set_viewport`); and `TreeView` (title, header line, footer banner, placeholder, body from the host's `Fn(&R, &RowCtx) -> Line` producer laid out for the window's rows only, the pending/ghost/selection looks, the scrollbar gutter, returning the window it drew).

Conspectus: `App` holds one `TreeState<RowId>` (also the whole `ViewStateSlot` tree state) in place of `expanded`, `selection`, `left_window`, and `last_visible_index`; `set_data`/`set_tree` share the rebuild routine; the three expand/collapse scans and the two ancestor walks are the state's; `rows::Row` implements `TreeRow`; `RowKind`/`RowId`, the builders, slot persistence, `recompute_explorer`, and status messages stay in the host. `draw_left_panel` builds a `TreeView`; `LeftRows::line` is the producer; `App::tree_row_states` works out each row's ghost, pending, and pin mark before the frame, so `pinned_nodes`, `presence`, and `row_is_frozen` no longer run in the renderer (a source check enforces it). No renderer sends a message (existing check). `Msg::PageDown`/`PageUp` lost their height payload and `translate` its height argument. ADR 0182 (amends 0031, 0156); page `crates/tuikit/docs/controls/tree_view.md`.

Verification: 771 `--snapshot` frames with ANSI (six views; stepping, ends, PgDn/PgUp, folds, regroup, view switches, focused and unfocused, status) at 100x30, 60x20, 40x10 are byte-identical before and after (`/tmp/tuifw-baseline/frames-750-*`). Framework unit tests plus hand-rolled property tests (6 seeds x 500 ops of rebuild/fold/step/page/jump/draw: the selection always shows and is valid, the window always shows the cursor, retention by id and by position). Host: new reducer test (retention keeps the duplicate's copy), new render test (window follows the cursor's copy); existing rows, ui_tests scroll, and consistency tests pass with only mechanical API edits (`last_visible_index`/`selection`/`expanded` field reads, `translate` without its height). fmt, clippy (workspace, all targets, all features, -D warnings), the isolated full suite, `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps`, `cargo build --examples`, and `git diff --check` pass.

**Regressions**
- None.

**Consistency wins**
- PageUp and PageDown move by the rows the tree showed in the last frame, and the window is clamped to the content (already from CSP-712 and CSP-724; verified unchanged in the frames).
- A row listed twice keeps the cursor on the copy it stood on across a refresh or a filter or sort change, and the window follows that copy instead of the first (no snapshot frame shows a duplicated row; covered by tests).
- One retention routine and one fold path for every rebuild and key.
<!-- SECTION:FINAL_SUMMARY:END -->
