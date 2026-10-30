---
id: CSP-758
title: 'Extract `TabBar`, `PriorityLine`, and `EmptyState`'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:27'
updated_date: '2026-10-30 14:26'
labels:
  - tui-framework
  - tuifw-chrome
milestone: m-25
dependencies:
  - CSP-723
  - CSP-725
  - CSP-757
  - CSP-716
  - CSP-790
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
priority: medium
type: enhancement
ordinal: 844000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The left pane's tab strip joins view labels with ` · ` and probably clips at 100 to 107 columns with no elision. The header shows chips with priority-drop fitting only in the status bar. Twelve hand-written empty-state messages share a slot but no code. Breadcrumb elision and tab elision are the same fit problem (inventory slice 07).

Scope: Create `TabBar` (active accented, rest secondary, separated, with elision), `PriorityLine` and `StatusChip` (pieces with priorities that drop in order to fit), and `EmptyState` (a message slot with a standard look). The header composition and the empty-state copy stay in Conspectus. Do not add a minimum-size screen (CSP-319 and CSP-190 stay open) but leave the seam.

TUI changes:
- Consistency wins: the tab strip fits or elides at every width; the header degrades by priority like the status bar
- Regressions or behavior changes: none expected

Tests and checks: `--snapshot` at widths 40, 60, 80, 100, 107, and 120 for the tab strip and header. Tests for each elision step.

Blockers: Text, chips, and the frame.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 `TabBar`, `PriorityLine`, `StatusChip`, and `EmptyState` are in the framework
- [x] #2 The tab strip and header never clip mid-label at any width from 40 to 120
- [x] #3 The 12 empty-state messages use `EmptyState` and keep their copy
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
1. Capture before frames (header, tab strip, status bar, every reachable empty state) at 40, 60, 80, 100, 107, 120, 160 into /tmp/tuifw-baseline/frames-758-before.
2. tuikit::text: let a `Piece` carry its own gap (cells before it in place of the separator) and add `elide_filled`, the strict rule plus a pass that fills the room back, highest rank first; extend the property test.
3. tuikit::tab_bar::TabBar on `elide` (strict, mark at the end): active tab never gives way (accent, bold), the rest from the end of the strip (inert), ` · ` between, ` …` plus a free cell when shortened; tests for each step; page tab_bar.md.
4. tuikit::priority_line::{PriorityLine, Segment} on `elide_filled`: ranked or fixed segments, a line separator and per-segment gaps, ` …` at the end; tests; page priority_line.md (with StatusChip pieces).
5. tuikit::empty_state::EmptyState: message plus optional hint, muted, wrapped, left or centered; `lines` and `render`; TreeView's placeholder draws through it; page empty_state.md; seam for a minimum-size screen.
6. Host: left_panel_title through TabBar (tab_strip_fit goes); header through PriorityLine with ranks (sessions fixed, freshness, mux, triage, stopped, opt-in chips); status segments through PriorityLine (byte-identical thanks to the fill pass); empty states of the left pane, right pane, and the Preview's nothing-to-show placeholders through EmptyState with their copy.
7. Tests: chrome consistency test for the header at every width 40..120; port tab strip tests; update expectations with notes.
8. After frames, diff by category; ADR 0190; design.md, architecture.md, README, CHANGELOG, operator-review, inventory verification.
9. fmt, clippy, isolated suite, rustdoc, examples, git diff --check; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Framework: `text::Piece::gap` and `text::elide_filled` (52126481), `TabBar` and `marks::MORE` (e908340d), `PriorityLine` and `Segment` (f527cad2), `EmptyState` and the tree's placeholder on it (a9169a0c). Host: tab strip, header, and status segments (461e5de6); empty states (69a8809c); docs and ADR 0190 (b5e8b2d6).
Refill decision: the fill pass was added (`elide_filled`, ties come back in order), so the status bar packs exactly as before; the strict rule would drop `Tab focus` at 107 columns (left focus). Status ranks are named and distinct in the old order.
Frames: 329 before/after in /tmp/tuifw-baseline/frames-758-{before,after} (diff in frames-758-diff.txt): 273 byte-identical, 38 header-only text changes (all row 0, at 40 columns and with harness chips below 115), 18 style-only (Mux view's Preview placeholders dimmed). Tab strip, status bar, and pane empty states byte-identical.
Page examples compiled and run as a throwaway test (removed). Full isolated suite 4677 passed, 0 failed, 1 ignored after one rerun (first run hit the known `discovery::forge` timing flake `a_cancelled_hosts_lookup_returns_promptly_with_no_answer`). Filed CSP-795 for the framework controls' own empty notes (LogList, Palette, Pager).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tuikit::tab_bar::TabBar` (on `text::elide`, mark at the end: inactive tabs go from the end, the active never, ` …` plus a free cell), `tuikit::priority_line::{PriorityLine, Segment}` (ranked or fixed segments, a separator or a segment's own gap, ` …`), and `tuikit::empty_state::{EmptyState, Align}` (message plus hint, wrapped, muted, left or centered; `TreeView`'s placeholder takes one). `StatusChip` stays in `tuikit::chip` (CSP-725) and is a segment like any other. `text::Piece` gained a gap, and `text::elide_filled` adds the fill pass the status bar needed: decision recorded in ADR 0190, with no visible effect (the strict rule would have dropped `Tab focus` at 107 columns). `marks::MORE` is the one line-end mark. Pages: tab_bar.md, priority_line.md, empty_state.md; text, glyphs, tree_view, app_frame, architecture, README updated.

Host: the tab strip is a `TabBar` (`tab_strip_fit` gone); the header is a `PriorityLine` (sessions fixed, then freshness, mux, `▲ N`, stopped, harness chips from the last to go); the status segments are a `PriorityLine` with named distinct ranks in the old order; the left pane's 15 empty states, the right pane's 2, and the Preview's nothing-to-show placeholders use `EmptyState` with their copy (the story's 12 was the inventory's count); Preview failures keep the plain look. No minimum-size screen; the seam is on the app frame and empty state pages.

Verified: 329 `--snapshot` frames before and after (header, status bar, whole screen, every view, both focuses, the reachable empty states, harness chips) at 40, 60, 80, 100, 107, 120, 160: 273 byte-identical, 38 header-only, 18 style-only. New tests: elide gap and fill (20,000-case property test), 14 TabBar, 13 PriorityLine (5,000-case packing check), 9 EmptyState, header at every width 40..120 in every view, harness chips whole. No existing expectation changed. fmt, clippy -D warnings, isolated suite 4677 passed / 0 failed / 1 ignored (one rerun after a `discovery::forge` flake), nextest 4677 passed, rustdoc -D warnings, `cargo build --examples`, `git diff --check`, independence guard, page examples compiled. Filed CSP-795.

**Regressions**
- With `show_harness_chips` on, a narrow header shows fewer chips: whole chips go from the end and the `▲ N` chip outranks them (100 columns: `claude 5 · codex 4 · ▲ 4 …` where it drew three chips and part of a fourth).
- The Preview's `loading mux preview…`, `no mux target for this row`, and its other nothing-to-show placeholders are now dim, so they read fainter.

**Consistency wins**
- The header never runs off the edge: at 40 columns it reads `updated 0s ago · 11 sessions · 4 mux …` where it cut `4 stopped` to `4`; it degrades by priority like the status bar, checked at every width 40..120.
- The tab strip and the status bar share the rule and draw byte for byte as before; the tab strip still never cuts a name.
- Empty previews recede like empty panes; failures stay readable.
- The right pane's `Select a row to view its detail.` wraps on a pane narrower than 34 columns instead of being clipped.
<!-- SECTION:FINAL_SUMMARY:END -->
