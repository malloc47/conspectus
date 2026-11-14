---
id: CSP-880
title: >-
  TUI: count each session once in the header line's warning count and harness
  chips
status: Done
assignee:
  - '@claude'
created_date: '2026-11-14 02:03'
updated_date: '2026-11-14 02:41'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 957000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The header line's `▲ N` (sessions with an ambiguous mux link) and the opt-in per-harness chips (`[tui] show_harness_chips`) count the session rows the view lists (`HeaderCounts::from_app` in `src/tui/ui/header.rs`), so in the PRs view a session listed under two pull requests of its checkout's branch counts twice. With a scratch copy of the showcase that adds a second open PR on `feature/extra`, the header reads `▲ 8` with or without a filter, over four ambiguous sessions (the showcase itself reads `▲ 4`). `N/M sessions` and the group headers' `(N)` and `▲` count each session once (CSP-879, CSP-856, ADR 0210's amendment); these two parts of the same line still count rows. Found while fixing CSP-879.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 In the PRs view a session listed under two pull requests counts once in the header line's ▲ N and in its harness chip
- [x] #2 The reference frames are unchanged where each session is listed once
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. In src/tui/ui/header.rs, add one helper that yields the sessions the row tree lists, each once by primary node (the rule CSP-856 gave group headers and CSP-879 gave N/M sessions), and use it for HeaderCounts::from_app (harness chips, the mux-state tallies, ▲ N) and for visible_agent_session_count's Sessions and PRs arms.
2. Unit test in src/tui/ui_tests.rs beside CSP-879's: a session under two pull requests counts once in ▲ N and in its harness chip, unfiltered and filtered.
3. Before/after header frames (every view, showcase and a two-PR copy, chips off and on, unfiltered and filtered, 170 and 70 columns) plus frames.sh: only the two-PR PRs view changes; reference frames unchanged.
4. docs/tui-columns.md (Header line: the harness chips and ▲ N count each session once), ADR 0210 amendment note, CHANGELOG.
5. checks.sh and just-check.sh, commit, finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Implemented: `listed_sessions` in src/tui/ui/header.rs yields the session rows each once by primary node; HeaderCounts::from_app (harness chips, ▲ N), visible_agent_session_count (Sessions, PRs) and compute_group_summaries all use it. New test `prs_view_header_chips_count_a_session_under_two_pull_requests_once` (fails before the fix with `claude 2 · ▲ 2`); CSP-879's test shares its fixture.
Frames (/tmp/csp-fu/hdr880.sh, frames.sh, prs879.sh; before = the CSP-879 binary): header pane in all six views on the showcase and the two-PR copy, chips off and on, unfiltered and --harness claude-code, at 170 and 70 columns (96 frames), plus the nine showcase views and the PRs view full-screen. Only the two-PR copy's PRs view changes: ▲ 8 -> ▲ 4, and with chips on claude 8 -> claude 4.
Validation: checks.sh clean; just check passed (5706 tests run, 5706 passed, 1 skipped).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The header line's `▲ N` and opt-in harness chips now count each listed session once, as `N/M sessions` and the group headers already did. Before, the PRs view counted a session under two pull requests of its checkout's branch twice (`▲ 8`, `claude 8` over four sessions on a two-PR copy of the showcase). One helper (`listed_sessions`) now holds the count-once rule for the header line and the group headers.

Regressions: none. Every showcase reference frame and every header frame where each session is listed once is byte-identical before and after (six views, two fixtures, chips off/on, filtered/unfiltered, 170 and 70 columns).
Consistency wins: on the two-PR copy the PRs view's header reads `▲ 4` and `claude 4`, agreeing with its `4/11 sessions` and its group headers' `(4)`.

Docs: tui-columns.md (Header line and PRs view), ADR 0210 amendment item 3, CHANGELOG. Verified with a new unit test, the frame comparisons, checks.sh, and just check (5706 passed).
<!-- SECTION:FINAL_SUMMARY:END -->
