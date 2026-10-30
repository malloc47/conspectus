---
id: CSP-791
title: >-
  Extract `StackedMasterDetail` and the section rule from the right pane's
  layout
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 08:09'
updated_date: '2026-10-30 10:26'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-755
documentation:
  - docs/plans/tui-framework/explorer-design.md
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 877000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`draw_right_panel` (`src/tui/ui/right_panel.rs`, about 100 lines) lays the right pane out inline: it lays the explorer's lines out one column narrower than the pane so a rule cannot wrap when the scrollbar appears, wraps them, sizes the explorer zone to its rows (at least three) while keeping six rows for the Preview when the pane allows, draws the one-row `Preview` rule, and keeps the zone's scroll window with the shared gutter. Only whole-frame tests reach that arithmetic (`right_pane_preview_keeps_minimum_height_when_related_full`). `chip_divider_line`, which draws the `Related` and `Preview` rules, lives in the host although it names nothing of the host. The explorer design (docs/plans/tui-framework/explorer-design.md, section 4.2) specifies `StackedMasterDetail` and a `SectionRule` for this; this is step 2 of its migration, and the explorer's zones then name their headings as section rules.

Scope: Add `tuikit::master_detail::StackedMasterDetail` (a list zone sized to its wrapped rows within a minimum and the room a minimum detail area leaves, a one-row divider, the detail area; the list drawn in a kept window with the shared gutter; the window and the areas returned) and `tuikit::frame::SectionRule` (a muted rule, a label chip in `accent` under `badge`, an optional summary before or after it, a two-cell closing rule), each with a control page. The right pane draws through both. Conspectus keeps what the zones hold: the explorer's lines, the search hit block above them, the Preview body, and the rules' words.

TUI changes:
- Consistency wins: one layout rule and one rule line, testable without the App
- Regressions or behavior changes: none; frames are byte-identical

Tests and checks: framework tests for the layout (the detail keeps its minimum when the list is long, the fallback on a short pane, the list's minimum, the layout width) and for both summary placements of the rule; the host's right-pane layout and scrollbar tests pass unchanged; before and after `conspectus tui --snapshot` frames of the right pane at 160x40, 100x30, and 60x20, focused and unfocused, compared byte for byte.

Blockers: the design (CSP-755).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `tuikit::master_detail::StackedMasterDetail` and `tuikit::frame::SectionRule` are in the framework as the explorer design specifies, and the right pane draws through them
- [x] #2 Framework tests cover the layout's minimum detail rows, its short-pane fallback, the list's minimum, the layout width, and both summary placements of the rule
- [x] #3 Before and after `conspectus tui --snapshot` frames of the right pane are byte-identical at 160x40, 100x30, and 60x20, focused and unfocused
- [x] #4 The framework crate names no Conspectus type, string, theme field, or domain word, and the independence guard passes
- [x] #5 Each control has a reference page in the framework docs: purpose, usage, key table, theme tokens used, and the house rules it follows
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Capture before frames of the right pane (160x40, 100x30, 60x20; focused and unfocused; workspace, repo, session, mux, PR, pin, history, fork, branch, search hit, drills, F full detail scrolled to its end, Other folded and open, edge meta) plus a few whole screens into /tmp/tuifw-baseline/frames-791-before.
2. Framework: add `tuikit::frame::SectionRule` (frame/rule.rs): muted rule, label chip in `accent` under `badge`, optional summary in `inert` after the chip or before it (`summary_first`), two-cell closing rule, leading rule drops out when there is no room; unit tests for both placements, no summary, and the narrow case.
3. Framework: add `tuikit::master_detail::StackedMasterDetail` and `Stacked` (min_list 3, min_detail 6 by default; `layout_width`, `areas`, `render` over `GridLines`, a divider line, and the kept `ScrollWindow`), with tests: the detail keeps its minimum when the list is long, the short-pane fallback, the list's minimum, the layout width, the wrap width at overflow, the window following the cursor line's rows, and the gutter.
4. Pages: `docs/controls/master_detail.md` from the template; SectionRule added to `docs/controls/frame.md`; module map, README, and frame module docs.
5. Host: `draw_right_panel` draws through `StackedMasterDetail` (MIN_PREVIEW_HEIGHT stays the host's constant), `related_rule` and `preview_divider_line` through `SectionRule`; `chip_divider_line` and `ChipAnchor` go. Host layout and scrollbar tests unchanged.
6. Capture after frames and compare byte for byte; record the result.
7. Docs: ADR 0188 status, design.md, explorer design migration step 2 as landed, operator review; run fmt, clippy, full isolated suite, doc, examples, diff --check; close the story.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Before frames captured from 3f8289d2 with /tmp/tuifw-baseline/scripts791/capture.sh: 261 frames (the right pane focused and unfocused for 40 states: workspace, repo, session, mux, PR, pin, history, fork, branch, search hits, the hit block above the zone, drills two and three deep, `F` full detail scrolled to its end, Other folded and open, edge meta; plus four whole screens) at 160x40, 100x30, and 60x20. 60x20 stacks the panes, so its right pane exercises the short-pane fallback.
Framework: `frame::SectionRule` (8 unit tests, one doctest) and `master_detail::StackedMasterDetail` (16 unit tests, one doctest). A probe against ratatui's solver showed `Layout::vertical([Length(l), Length(1), Min(0)])` gives (min(l, h-1), 1, rest) for h >= 2 and (1, 0, 0) at h = 1; `areas` reproduces that with arithmetic. The page examples were compiled and run as a scratch integration test (kept at /tmp/tuifw-baseline/scripts791/page_examples.rs, not committed).
Host: `draw_right_panel` draws through one `LAYOUT` constant; both rules are `SectionRule`s; `chip_divider_line`, `ChipAnchor`, the unused `App` parameter of `preview_divider_line`, and the host's `ScrollGutter`/`WrappedRows` imports went.
After frames: all 261 byte-identical (`diff -r -q` over .ansi and .err). Host tests `right_pane_preview_keeps_minimum_height_when_related_full`, `right_pane_scrolls_to_keep_explorer_cursor_visible`, and the two right-pane scrollbar tests pass unchanged.
Found while probing: at 40 columns the `Related` rule wraps onto a second row (pre-existing; the rule does not give way below its own width). Filed CSP-794.
Checks: fmt --check ok; clippy --workspace --all-targets --all-features -D warnings ok; isolated full suite 4556 passed, 0 failed, 1 ignored; nextest (isolated) 4556 passed, 1 skipped; tuikit doctests 112 passed; RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps ok; cargo build --examples ok; git diff --check ok; independence guard ok.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed `tuikit::frame::SectionRule` (`new(label)`, `.summary(text)`, `.summary_first()`, `.line(width, &theme)`, `CLOSING`: a rule in `muted`, the label as a chip in `accent` under `badge`, an optional summary in `inert` after the chip or before it, a two-cell closing rule, the leading rule dropping out when there is no room) and `tuikit::master_detail::StackedMasterDetail` with `Stacked` (`min_list` 3 and `min_detail` 6 by default; `layout_width`, one column short of the pane; `areas`; `render` over `GridLines`, a divider line, and the kept `ScrollWindow`, returning the window and the three areas). Pages: `crates/tuikit/docs/controls/master_detail.md` (new) and `frame.md` (now `ModalFrame` and `SectionRule`); module map, README, and frame module docs updated.

The right pane draws through both: `draw_right_panel` lays its lines out at `layout_width`, splices the hit block as before, and renders through one `LAYOUT` constant (the host keeps `MIN_PREVIEW_HEIGHT`); the `Related` (summary first) and `Preview` rules are section rules. `chip_divider_line`, `ChipAnchor`, and `preview_divider_line`'s unused `App` went. Conspectus keeps the lines, the hit block, the Preview, and the rules' words.

Departures from the sketch, in the operator review: the split is arithmetic matching ratatui's solver (including under four rows: the list's first row, then the divider's, then the rest of the list); an empty summary shows nothing (the host drew two spaces; nothing passed one); the list's room stays private (a seam).

Verified: 24 new framework tests (16 layout: the detail's minimum with a long list, the short-pane fallback, the list's minimum, tiny areas, parameters, the layout width, a rule at the layout width staying one row when the bar appears, full-width wrap while fitting, narrower wrap on overflow, the window following a wrapped cursor line, the kept window, no cursor, the divider row, fade on fit; 8 rule: both summary placements, the chip's column, no summary, cells, the dropped leading rule, tokens) and two doctests; the host's right-pane layout and scrollbar tests unchanged and passing; 261 before and after `conspectus tui --snapshot` frames (right pane focused and unfocused over 40 states, plus whole screens, at 160x40, 100x30, 60x20) byte-identical. fmt, clippy -D warnings, the isolated suite (4556 passed, 0 failed, 1 ignored), nextest, tuikit doctests, cargo doc -D warnings, examples, git diff --check, and the independence guard pass.

**Regressions**: none; frames are byte-identical.

**Consistency wins**: one layout rule and one rule line, testable without the App; the anchor enum whose docs contradicted the code is gone.

Follow-up filed: CSP-794 (a section rule does not give way below its own width, so the `Related` rule wraps onto a second row at 40 columns).
<!-- SECTION:FINAL_SUMMARY:END -->
