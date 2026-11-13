---
id: CSP-724
title: 'Extract `scroll`: cursor stepping, follow offset, and the scrollbar gutter'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 15:39'
labels:
  - tui-framework
  - tuifw-foundation
milestone: m-25
dependencies:
  - CSP-718
  - CSP-720
  - CSP-712
documentation:
  - docs/plans/tui-framework/inventory/01-design-tokens.md
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 810000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Cursor stepping lives in `cursor.rs` (wrap in lists, clamp in the tree, explorer, and menus), scroll-to-cursor is copied five times, and the scrollbar has five call sites, one a copy in `widgets/help.rs`. Scroll offsets are written back by the renderer (`Cell`s on `App`) and by messages dispatched during `draw`. Wrapped-row counting has four implementations (inventory slices 01 and 07).

Scope: Create `tuikit::scroll` with `wrap_step`, `clamp_step`, a follow-cursor offset function, a scroll window type that clamps on use, the fade-on-fit scrollbar gutter, and one wrapped-row counter. Migrate the five scroll-to-cursor copies, the five scrollbar sites, and the four wrapped-row counters. The renderer reports the measured viewport to the owner through a return value, not a dispatched message.

TUI changes:
- Consistency wins: offsets are always clamped, so no blank tail; one scrollbar look everywhere
- Regressions or behavior changes: none expected

Tests and checks: Property tests: the cursor is always inside the window after any step sequence; the window is always inside the content. Existing scroll tests in `ui_tests.rs` pass.

Blockers: The scroll and paging defect task, so the fixes land before the code moves.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `tuikit::scroll` provides stepping, a follow offset, a clamping scroll window, the scrollbar gutter, and one wrapped-row counter
- [x] #2 The five scroll-to-cursor copies, the five scrollbar sites, and the four wrapped-row counters use it
- [x] #3 No renderer dispatches a message during `draw` to report a viewport
- [x] #4 Property tests cover the cursor and window invariants
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
1. Add `tuikit::scroll`: `wrap_step` and `clamp_step` (moved from `src/tui/cursor.rs`), `follow_offset` (minimal motion over a row span; the start of a span taller than the window wins), `ScrollWindow` (offset, viewport, content; every constructor and step clamps), `ScrollGutter` (the 1-column right gutter carved only on overflow, and the one scrollbar recipe: thumb reaches the bottom at the last offset, 1-row top and bottom margin, no inherited REVERSED), and `WrappedRows` (lines wrapped by `text::wrap_line`, with each line's row range, so a count is the count drawn). Unit tests plus hand-rolled property tests with a fixed-seed generator: the cursor stays in the window and the window in the content after any step, page, home, end, and resize sequence.
2. Migrate stepping: every `cursor::wrap_step`/`clamp_step` caller imports `tuikit::scroll`; delete `cursor.rs`.
3. Migrate the scroll-to-cursor copies (form, controls, multi-select, search x2, picker, plus the Messages list and the row menu) to `follow_offset`; the App's `adjust_left_scroll` and `adjust_explorer_scroll` become `ScrollWindow`s.
4. Migrate the five scrollbar sites (left pane, explorer, preview plain and windowed, the help page copy) to `ScrollGutter`, and the wrapped-row counting (`ui::wrap_lines`, `wrap_into`, the explorer's per-line rows, the preview and Related banners, the left banner) to `WrappedRows`. Replace `popup_frame::ScrollFit` with `ScrollWindow`.
5. Replace `Msg::LeftViewportChanged`/`ExplorerViewportChanged`: the panels compute their window from `&App` and the measured area, draw it, and return it; `ui::draw` returns a `ViewportReport` (tree, explorer, preview, help, value modal, Messages) that the caller hands to `App::keep_viewports` after the frame. Drop the arm special case in the reducer; keep its regression test by drawing a frame between presses.
6. Docs: `crates/tuikit/docs/controls/scroll.md`, module map, README; ADR 0156 (scroll window, follow rule, gutter, viewports by value; amends ADR 0076 and ADR 0085 contract 5); design.md; operator-review entries; changelog if visible.
7. Verify: before/after `--snapshot` frames (normal and narrow), fmt, clippy, isolated suite, docs, diff-check.

8. Keep the Preview's offset while its content loads (a placeholder's window would take it back to the top), so keys pressed during a load and `--snapshot-keys` still scroll.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in six commits: 4233a68 (tuikit::scroll with property tests, control page, module map), 2e01a4d (stepping callers and the stateless follow copies; cursor.rs deleted), fe4698c (ScrollGutter at the five scrollbar sites, WrappedRows for the row counts, ScrollFit replaced by ScrollWindow), aa577f8 (ViewportReport and App::keep_viewports replace the two viewport Msgs; the arm exemption is gone), 5d1f104 (the Preview keeps its offset while loading), 34be29a (ADR 0156, design, changelog, operator review).

The story assumed Cell offsets on App; CSP-499 had already replaced them with LeftViewportChanged/ExplorerViewportChanged sent from the draw. Those messages are removed; draw returns the windows by value. draw still takes &mut App only for prepare_preview.

Frames: /tmp/tuifw-baseline/frames-724-before and -after (104 scenarios x 5 sizes x 2 fixtures = 1040 frames, capture-724.sh, diff-724.sh, list in frames-724-diff.txt). 24 differ, all Preview overscroll cases (preview-plain-over, preview-plain-back, preview-mux-over at 100x30, 80x24, 60x24, 40x20, both fixtures): a blank tail became the last page. No default frame changed. The value modal is not reachable from the fixture (no truncated value); its behavior is covered by the value_modal and scrolling tests.

Checks: fmt ok; clippy --workspace --all-targets --all-features -D warnings ok; isolated suite 3712 passed, 0 failed, 1 ignored; nextest 3712 passed after one rerun (first run: cli_history::rebuild_plan_of_a_running_mux_is_a_pasteable_script, "server exited unexpectedly", a tmux flake); RUSTDOCFLAGS=-D warnings cargo doc --workspace ok; git diff --check ok; tuikit independence 35 passed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::scroll` (ADR 0156): `wrap_step` and `clamp_step` (moved from `src/tui/cursor.rs`, now deleted), `follow_offset` (minimal motion; a span taller than the window shows its start), `ScrollWindow` (clamps on every constructor and step; replaces `ScrollFit`), `ScrollGutter` (fade-on-fit 1-column gutter, one scrollbar recipe with the 1-row margin), and `WrappedRows` (rows wrapped by `text::wrap_line` with each line's span). Hand-rolled property tests drive step, page, home, end, and resize sequences: the cursor stays in the window, the window in the content, and a cursor moving inside the window does not scroll it. Page: `crates/tuikit/docs/controls/scroll.md`.

Migrated: every stepping caller; the scroll-to-cursor copies (forms, Controls, multi-select, search x2, picker, Messages list, row menu) as `ScrollWindow::new(..).following(cursor)`, which equals their old formula (tested exhaustively); `adjust_left_scroll` and `adjust_explorer_scroll` as kept windows; the five scrollbar sites including the help copy; `ui::wrap_lines`, `wrap_into`, the explorer's per-line counts, and the banner counts. The draw sends no `Msg`: `ui::draw` returns a `ViewportReport` that `App::keep_viewports` applies after the frame; `LeftViewportChanged`, `ExplorerViewportChanged`, and the reducer's arm exemption are gone (a source test keeps it so).

Verified: fmt, clippy `-D warnings`, isolated suite 3712 passed / 0 failed / 1 ignored, nextest 3712 passed (after one tmux flake rerun), docs `-D warnings`, `git diff --check`, independence guard; `--snapshot` frames before and after at 100x30, 80x24, 60x24, 40x20, and 40x12, focused and unfocused (`/tmp/tuifw-baseline/frames-724-*`): only the 24 Preview overscroll frames differ.

**Regressions**
- None found. The three menus still clamp (CSP-727 makes them wrap).

**Consistency wins**
- A plain or pane Preview scrolled past its end shows its last page instead of blank rows, and the first `J`/`K` back moves it at once (rule 12).
- The explorer fills its zone after its rows shrink instead of the last row over blank rows (rule 12).
- A cursor line taller than the explorer zone shows its first rows (its label), as the row menu did.
- The transcript Preview's thumb is placed by the transcript's own rows under a banner or Related header.
<!-- SECTION:FINAL_SUMMARY:END -->
