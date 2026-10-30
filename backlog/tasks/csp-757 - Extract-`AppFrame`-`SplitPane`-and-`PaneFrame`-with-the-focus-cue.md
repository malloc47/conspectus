---
id: CSP-757
title: 'Extract `AppFrame`, `SplitPane`, and `PaneFrame` with the focus cue'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 13:13'
labels:
  - tui-framework
  - tuifw-chrome
milestone: m-25
dependencies:
  - CSP-726
  - CSP-721
  - CSP-729
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
  - docs/adr/0076-scrollbar-widget-choice.md
priority: medium
type: enhancement
ordinal: 843000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`ui::draw` is a screen shell that takes `&mut App` and dispatches viewport messages so the reducer can reconcile scroll offsets. The body split is copied three times (`ui.rs`, `snapshot.rs`, and test fixtures). Focus is shown only by a `► ` in the pane title: borders never change, whereas modal borders always use the accent, and ADR 0076 says the focused pane's border is highlighted. `runtime.rs:843` is a three-way switch above the chrome (inventory slice 07, section 1).

Scope: Create `AppFrame` (header row, body, footer row, overlay layer, toast layer), `SplitPane` (side by side above a width threshold, stacked below), and `PaneFrame` with a focus marker and an accent-tinted border. The snapshot code and test fixtures use the framework's split. The renderer returns measured sizes instead of dispatching messages. The header composition and which view fills the body stay in Conspectus.

TUI changes:
- Consistency wins: the focused pane is visible at a glance; one split implementation; the pane and modal frames share one vocabulary
- Regressions or behavior changes: the focused pane's border and title marker take the accent colour; the unfocused pane keeps the plain border

Tests and checks: `--snapshot` of the main screen with each pane focused, at wide and stacked widths. The snapshot driver and tests use the same split.

Blockers: The modal frame and the overlay contract.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `AppFrame`, `SplitPane`, and `PaneFrame` are in the framework and the body split exists once
- [x] #2 The focused pane's border and title marker use the accent colour, matching ADR 0076
- [x] #3 `ui::draw` no longer dispatches messages
- [x] #4 `snapshot.rs` and test fixtures use the framework's split
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
1. Capture before frames (showcase fixture, each view, each pane focused, 160x40, 100x30, 80x24, 60x40; panes all/left/right/header/status; a menu over the panes, a drilled right title, the viewer) into /tmp/tuifw-baseline/frames-757-before.
2. tuikit `frame`: `PaneFrame` (bordered pane, the lead of a space and the focus marker, the host's title in the modal title form with the closing pad only when the border has room, border and marker in `panel_focus_accent` when focused, title text untinted) and `SplitPane` (halves side by side at or above a width threshold, stacked below); the title-form helpers shared with `ModalFrame`. `TreeView` draws its pane through `PaneFrame`.
3. tuikit `app_frame`: `AppFrame` (header rows, body, footer rows; `Screen` trait the host implements for each part, the overlay layer drawn over them and skipping the chrome when an overlay fills the screen, the toast layer last; `areas` for the layout; returns the body's report).
4. Conspectus: `ui::draw` through `AppFrame` with a `MainScreen` adapter; draw helpers take a `Buffer`; the body split through `SplitPane`; the right pane through `PaneFrame`; the titles lose the lead and pad; one `ui::screen_areas` used by the draw, `--snapshot-pane`, and the test fixtures (ui_tests, consistency fixtures, selection, styling, rendering).
5. Docs: frame.md (PaneFrame, SplitPane), new app_frame.md, tree_view.md, theme.md, architecture.md, operations.md focus lines, ADR 0189 (+ index, amend 0076/0158 status), design.md, CHANGELOG, operator-review rows.
6. After frames, diff by category; fmt, clippy, full isolated suite, doc, examples, diff --check; finalize backlog.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed in four commits: `PaneFrame` and `SplitPane` in `tuikit::frame` (the title-form helpers now shared with `ModalFrame`), `AppFrame` with the `Screen` trait in `tuikit::app_frame`, the host adoption (`ui::draw` through `AppFrame` over a `MainScreen` adapter, the body through `SplitPane`, both panes in a `PaneFrame`, `TreeView` framing itself with one, `ui::screen_areas` for `--snapshot-pane` and the tests), and the docs (ADR 0189, frame.md, app_frame.md, tree_view.md, theme.md, architecture.md, README, operations.md, tui-columns.md, design.md, CHANGELOG, operator-review rows).

The `draw_screen` three-way switch the inventory found was already gone (the viewers are stack entries that fill the screen); `AppFrame`'s `overlays_fill_screen` is that switch now and `draw_screen` stays the wrapper that keeps the report. `ui::draw` already returned a `ViewportReport`; `the_renderer_sends_no_messages` keeps it so.

The test kit's `frame_rect` needed one fix: at 40x10 a modal that fills the body shares its top (or bottom) corners with the focused pane's, now tinted the same, so only one side's corners differed; it now completes the frame from the corners `with` draws across. Two scrolling checks (`every_modal_shows_its_body_at_40x10`, `menus_draw_on_a_terminal_shorter_than_their_frame`) caught it.

Frames: 584 before/after captures at 160x40, 100x30, 80x24, 60x40 (every view, each pane focused, each `--snapshot-pane`, drilled titles, a fork, menus, Controls, help, Messages, quick select, the viewer, a toast, a status message) in /tmp/tuifw-baseline/frames-757-{before,after}; 296 byte-identical, 288 differ only by cyan (36) added to the focused pane's border glyphs (6 of them partly under a wide modal); summary in /tmp/tuifw-baseline/frames-757-diff.txt.

Checks: fmt, clippy (workspace, all targets and features, -D warnings), the isolated suite (4630 passed, 0 failed, 1 ignored), nextest (4630 passed, 1 skipped), tuikit doctests (111), RUSTDOCFLAGS='-D warnings' cargo doc, cargo build --examples, the independence guard, git diff --check: all pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The screen, the panes, and the split are framework controls, and the focused pane is framed in the focus accent (house rule 9, ADR 0189).

- `tuikit::frame::PaneFrame`: a bordered pane whose top border holds a lead (a space, the focus marker, a space) and the host's title in the modal title's form; the border and the marker take `panel_focus_accent` while the pane has focus, the title's own text keeps its looks, and an unfocused pane keeps the plain border. `TreeView` draws its pane in one.
- `tuikit::frame::SplitPane`: halves side by side at or above a width threshold, stacked below; the one split, used by the draw, `--snapshot-pane` (through `ui::screen_areas`), and the tests (the copies in `snapshot.rs`, `ui_tests.rs`, and the consistency fixtures are gone).
- `tuikit::app_frame::AppFrame` with a `Screen` trait: header row, body, footer row, then the host's modal stack bottom to top, then the toasts, the chrome skipped while an overlay fills the screen, the body's `ViewportReport` handed back with nothing dispatched. `ui::draw` is an `AppFrame` over a `MainScreen` adapter; the header's words, the titles, and which view fills the body stay in Conspectus.

**Regressions**
- None beyond the intended change: the focused pane's border (corners, sides, bottom, and the top border's rule) is now drawn in `panel_focus_accent`, cyan by default and following `accent`, also while a modal is open over it. A custom `[tui.theme] accent` or `panel_focus_accent` now recolors that border too.

**Consistency wins**
- The focused pane is visible at a glance: its border and its `►` share the accent, as ADR 0076 always claimed; the unfocused pane keeps the plain border and a blank marker cell.
- Pane frames and modal frames share one title form and one module.
- One split implementation for the draw, the snapshot tool, and the tests.

Verified with `conspectus tui --snapshot` on the showcase fixture: 584 frames at 160x40 and 100x30 (side by side) and 80x24 and 60x40 (stacked), each pane focused, whole screens and each `--snapshot-pane`; 296 byte-identical (headers, status bars, unfocused panes, the full-screen viewer) and 288 differing only by the tint on the focused pane's border cells (6 of them partly under a wide modal). Tests: pane, split, and app-frame unit tests; `only_the_focused_pane_is_framed_in_the_focus_accent` (every view, both layouts, both focuses); the full isolated suite (4630 passed, 0 failed, 1 ignored), nextest, fmt, clippy, rustdoc with -D warnings, examples, the independence guard, and `git diff --check`.

Review: the cue follows the pane's focus also under an open modal, the title text is not tinted, and the unfocused blank marker keeps the accent style (all in docs/plans/tui-framework/operator-review.md).
<!-- SECTION:FINAL_SUMMARY:END -->
