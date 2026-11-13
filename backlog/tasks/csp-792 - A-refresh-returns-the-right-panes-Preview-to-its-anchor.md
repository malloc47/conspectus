---
id: CSP-792
title: A refresh returns the right pane's Preview to its anchor
status: Done
assignee:
  - '@claude'
created_date: '2026-10-30 08:49'
updated_date: '2026-10-31 00:45'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
type: bug
ordinal: 878000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Found while fixing CSP-789. `App::recompute_explorer` calls `reset_preview_scroll` before anything else, and every `SetData` (the periodic refresh, `r`) and every projection rebuild (`SetTree`: filter and sort changes) runs it. So an operator who has scrolled the Preview with `J`/`K` (up a transcript from its newest turns, or down a pane capture or a PR body) is put back at the anchor on the next refresh, even when the left selection, the explorer's focused node, and its cursor did not change. `App::update` already resets the Preview when what it is pointed at changes (`preview_focus`), which suggests the unconditional reset predates that rule rather than being intended. Not yet confirmed with a reducer test; confirm first, then decide against ADR 0181's anchoring rules.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A reducer test scrolls the Preview, delivers a refresh with the same snapshot and selection, and finds the scroll kept; or the task records why the reset is intended and closes
- [x] #2 A refresh that changes what the Preview shows (another selection, focused node, or cursor row) still starts it at its anchor
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Confirm with a reducer test (app_tests.rs, beside the CSP-789 refresh tests): scroll the Preview, deliver `SetData` with the same snapshot and selection, then a sort and a filter change, and a drilled pane; expect the scroll kept. Record the result.
2. Decide against ADR 0181: its `ScrollRegion` keeps its distance from the anchor as rows arrive (decision 5), and `App::update` already returns the Preview to its anchor when what it is pointed at changes, so the unconditional reset in `recompute_explorer` is not intended. Remove it, leaving the one rule in `App::update` (plus the explicit resets: drill, back, wrap and session-preview settings).
3. Make the rule see a cursor row that changes in place: `preview_focus` compares the explorer cursor's row key as well as its position, so a refresh that puts another row under the cursor at the same position still starts the Preview at its anchor (only adds resets; no gesture that reset before stops resetting, apart from the refresh paths).
4. AC2 test: a refresh that moves the left selection, changes the explorer's focused node (drilled node gone), or puts another row under the cursor starts the Preview at its anchor.
5. Run the existing preview-scroll tests; record visible changes (keys that do not move the selection and a jump to the row already selected now keep the scroll too) in operator-review, operations.md, and CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Confirmed: `a_refresh_keeps_the_preview_where_it_was_scrolled` failed before the fix (the Preview, scrolled 3 rows on the session's mux row, read 0 after `SetData` with the same snapshot and selection).

Fixed: removed the reset at the top of `recompute_explorer`; `App::update`'s `preview_focus` rule decides, now comparing the cursor's row key as well as its position (with the reset removed alone, the AC2 case "another row at the cursor's position" kept the stale scroll: verified failing, then passing with the key). Tests: `a_refresh_keeps_the_preview_where_it_was_scrolled` (refresh, sort and filter change, a non-moving key, a jump to the same row, a drilled pane) and `a_refresh_that_changes_what_the_preview_shows_starts_it_at_its_anchor` (another selection, focused node, cursor row). All 173 preview tests pass. Snapshot mode cannot show a same-world refresh (`r` there runs a real discovery; the captured frames in /tmp/tuifw-baseline/frames-792-* are byte-identical before and after), so the reducer tests are the evidence. Full isolated suite 4801 passed, 0 failed, 1 ignored; fmt, both clippy runs, doc, examples, diff --check pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Confirmed with a reducer test, then fixed: the reset was not intended. `recompute_explorer` returned the Preview to its anchor on every `SetData` and `SetTree`, so the periodic refresh, `r`, and a filter or sort change undid `J`/`K` with nothing else moving. ADR 0181's anchors keep their distance as rows arrive, and `App::update` already resets the Preview when what it is pointed at changes, so the unconditional reset is gone and that one rule decides.

- `preview_focus` (now the `PreviewFocus` type) compares the explorer cursor's row key as well as its position: a refresh that puts another row at the cursor's position (the session's mux replaced) still starts the Preview at its anchor. It only adds resets.
- Tests in `app_tests.rs`: `a_refresh_keeps_the_preview_where_it_was_scrolled` and `a_refresh_that_changes_what_the_preview_shows_starts_it_at_its_anchor`.
- Docs: operations guide (right-pane keys), changelog, a status note in ADR 0181, operator-review row.

**Regressions:** none.

**Consistency wins:**
- A refresh, `r`, and a filter or sort change keep a Preview scrolled with `J`/`K` where it was (up a transcript, down a pane capture or a PR body), on a drilled pane too.
- A key that does not move the selection (`j` on the last row) and a jump to the row already selected (no drill) keep it too; they returned it to its anchor.

Not visible in `conspectus tui --snapshot`: `r` there runs a real discovery rather than reloading the fixture, so the before and after captures (`/tmp/tuifw-baseline/frames-792-*`, 100x30 and 50x20) are byte-identical; the reducer tests are the evidence. Full isolated suite 4801 passed, 0 failed, 1 ignored; fmt, both clippy runs, doc, examples, and `git diff --check` pass.
<!-- SECTION:FINAL_SUMMARY:END -->
