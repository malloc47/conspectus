---
id: CSP-716
title: Reproduce and fix the suspected chrome and fixture-mode defects
status: Done
assignee:
  - '@claude'
created_date: '2026-10-27 20:26'
updated_date: '2026-10-28 09:31'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
priority: medium
type: bug
ordinal: 802000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Three chrome problems were found by reading (slices 06 and 07). The tab strip probably clips at 100 to 107 columns with no elision. The header probably reads `0/N sessions` outside the Sessions and Mux views. In `tui --fixture` interactive mode, a menu opened with `.` may not be dismissable, because `StaticMode::dispatch` answers every menu and form key forwarder with a status hint. The framework's `TabBar`, `StatusBar`, and `testkit` should be built from known behavior.

Scope: Reproduce each. Fix the ones that reproduce in the current code, or decide that fixture-mode interaction is dead and remove it, recording the decision. Record those that do not reproduce.

TUI changes:
- Consistency wins: the tab strip fits or elides at every width; the header count matches the active view; fixture mode is either usable or removed
- Regressions or behavior changes: none expected

Tests and checks: Snapshots at widths 90, 100, 104, 107, and 120. A test for the header count in each view.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Each suspected defect is reproduced and fixed, or recorded as not reproducible
- [x] #2 The tab strip is checked at widths 90, 100, 104, 107, and 120 and never clips mid-label
- [x] #3 The header count reflects the active view
- [x] #4 Fixture-mode interaction can dismiss every overlay, or its interactive mode is removed with the decision recorded
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce each suspect: tab strip via `tui --snapshot` on the showcase fixture at 40-120 columns in all six views; header count in each view; fixture mode in a pty (`tui --fixture`, `j . Esc Ctrl-C q`).
2. Tab strip: make `left_panel_title` fit its pane: full labels when they fit, otherwise drop inactive labels from the far end of the strip and end it with `…`, keeping the active label; measured by display width. Tests at every width 40..=120 in every view (no clipped label, active visible, `…` when shortened), plus snapshots at 90/100/104/107/120.
3. Header count: generalize the Mux rule (plain total unless something narrows the view) to PRs, Pinned, History, and Search; Search counts the sessions its search lists. A test per view.
4. Fixture mode: interactive fixture mode is used (`just demo`, README, dev-scenarios, ADR 0069), so fix it: menus, prompts, and forms take their keys in static mode as in the live TUI, their picks run on the pure executor (which refuses live-only effects with a toast). Share the overlay key handlers between both modes via a send callback. Behavior test: every modal closes on Esc and Ctrl-C in fixture mode; `.` then Esc closes the row menu.
5. Docs: tui-columns.md / operations.md where they describe the strip or the count; inventory Verification (CSP-716) subsections; operator-review rows for judgment calls.
6. Checks: fmt, clippy, full isolated suite, rustdoc, git diff --check.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Reproduced all three. Tab strip: `tui --snapshot` on the showcase fixture at 40..120 columns in all six views clipped mid-label at 100-104 side by side (`sea`..`searc`) and at 52 and under stacked (`his` at 40), Search's own name included. Header: Pinned, History, Search (before a search) read `0/11 sessions`, PRs `4/11`. Fixture mode: a pty run of `tui --fixture tests/fixtures/showcase.json` opened the row menu with `.`; Esc, Ctrl-C, and q were all answered with the static hint and the process had to be killed. Interactive fixture mode is used (`just demo`, README, dev-scenarios, ADRs 0069/0070, `dev scenario tui`), so it was fixed, not removed.
Commits: cfcfe93 header count, 67646ea tab strip, 1e03474 fixture mode, f89f696 docs. The new tests failed 8 of 9 against the old code (the PR-filter case already held).
Checks: fmt, clippy -D warnings, isolated cargo test (3525 passed, 0 failed, 1 ignored), nextest (3525 passed, 1 skipped), rustdoc -D warnings, git diff --check. One run of the tab-strip-only state saw `layout::nothing_a_modal_draws_escapes_its_frame` fail once; it passed on rerun and 8 further runs.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
All three suspected defects reproduced and are fixed.

- **Tab strip** (`src/tui/ui/left_panel.rs`): the 52-cell strip was clipped mid-label by the pane border at 100-104 columns side by side and at 52 and under stacked, the active name included. `tab_strip_fit` now names every view when they fit; otherwise inactive names drop from the end, the active name stays, and the strip ends with ` …`. Checked at every width 40..120, both layouts, every view, both focus states (`consistency_tests/chrome.rs`), plus an insta snapshot at 90, 100, 104, 107, 120.
- **Header count** (`src/tui/ui/header.rs`): Pinned, History, and Search before a search read `0/N sessions`, PRs `4/N` unfiltered. `N/` now appears only when the view lists fewer sessions than there are: Sessions counts its rows, Search the sessions its search found, Mux and PRs the plain total until a filter, the content search, pinned-only, or a PR filter narrows them, Pinned and History always the plain total. A test per view, filtered and unfiltered.
- **Fixture mode** (`src/tui/runtime.rs`, `runtime/overlay_keys.rs`): it is used (`just demo`, README, dev-scenarios), so it was fixed. A menu opened with `.` (or `N`, or a menu pick) refused every key, Esc and Ctrl-C included, and the process had to be killed (pty run). The nine menu/prompt/form handlers take a `send` callback; live mode passes the live executor, static mode the pure one, which refuses live-only picks with a toast. `consistency_tests/fixture_mode.rs`: every built modal closes on Esc and Ctrl-C.

Docs: Verification (CSP-716) in inventory 07 and 06, CHANGELOG, dev-scenarios, three operator-review rows, re-rendered Pinned/History/PRs screenshots.

Verified: fmt, clippy -D warnings, isolated suite 3525 passed / 0 failed / 1 ignored, nextest 3525 passed, rustdoc -D warnings, git diff --check; snapshot frames and pty runs before and after.

**Regressions**
- None. Intended behavior changes: the tab strip shortens with ` …` where it used to clip; the header reads `N sessions` instead of `0/N` or `4/N` in PRs, Pinned, History, and Search before a search (the Pinned, History, and slot-menu snapshots were updated for it); Mux's count now also follows the content search and pinned-only; in fixture mode a menu pick that needs the live TUI shows "that isn't available in this mode" instead of the static hint.

**Consistency wins**
- The tab strip fits or elides at every width and never cuts a name; the active view's name always shows.
- The header count follows one rule in all six views and matches what the view lists.
- Fixture and scenario overlays take the same keys as the live TUI and close on Esc and Ctrl-C; `.`-then-Pin and `p` are refused the same way.
<!-- SECTION:FINAL_SUMMARY:END -->
