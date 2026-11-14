---
id: CSP-879
title: 'TUI: count each session once in the PRs view''s N/M sessions'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-14 01:04'
updated_date: '2026-11-14 02:06'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 956000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Under a narrowing filter the header's `N/M sessions` in the PRs view counts the session rows the view lists (`visible_agent_session_count` in `src/tui/ui/header.rs`), so a session listed under two pull requests of one checkout counts twice and N can overstate what is listed, or even exceed M. Group headers count each session once since CSP-856 (ADR 0210, amended); the header line should agree on what one session is. Found while fixing CSP-856.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Under a narrowing filter, a session listed under two pull requests counts once in the PRs view's N
- [x] #2 N is unchanged where each session is listed once (the reference frames are unchanged)
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. In `visible_agent_session_count` (src/tui/ui/header.rs), count the distinct sessions (by the row's primary node, as group headers count them since the ADR 0210 amendment) that the PRs view's session rows stand for under a narrowing filter, instead of the rows. Sessions view: same helper (each session is listed once there, so its N is unchanged).
2. Test in ui_tests.rs: a PRs-view world with one checkout, two open PRs from its branch, one session in the checkout and one elsewhere; under `--harness`-style filter the header reads `1/2 sessions`, not `2/2`.
3. Before/after frames: frames.sh (nine showcase views) and PRs view on the showcase and on a scratch copy with a second PR on `main`, unfiltered and with a harness filter, at 170 and 70 columns.
4. Docs: header doc comment, tui-columns.md if it states the header rule, ADR 0210 amendment note (header line agrees), CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Before/after frames (binaries built with --features snapshot at 3bd85e12 and with the fix): frames.sh's nine showcase views identical; PRs view (`--view prs`) on the showcase, unfiltered and with `--harness claude-code`, at 170 and 70 columns identical; on a scratch copy of the showcase with a second open PR (#9) on `feature/extra`, unfiltered identical, filtered header `8/11 sessions` -> `4/11 sessions` at both widths, nothing else in the frame changed. The same line's `▲ N` still reads 8 there (it and the harness chips count rows); filed as CSP-880. Sessions view uses the same distinct count; it lists each session once, so its frames are unchanged.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`visible_agent_session_count` (src/tui/ui/header.rs) counts the distinct sessions (by primary node, as group headers count them) that the PRs view's session rows stand for under a narrowing filter, instead of the rows; the Sessions view uses the same count. Test `prs_view_header_counts_a_session_under_two_pull_requests_once` (ui_tests.rs): one checkout, two open PRs from its branch, a Claude Code session in the checkout listed under both, under a harness filter the header reads `1/2 sessions` (it read `2/2` before).

Visible change: on a scratch showcase copy with a second PR on `feature/extra`, the filtered PRs header goes from `8/11 sessions` to `4/11 sessions` (170 and 70 columns); every showcase frame (frames.sh's nine views, PRs view filtered and unfiltered at 170 and 70) is unchanged. Regressions: none seen. Consistency wins: the header line's N now agrees with the group header's `(N)` on what one session is. tui-columns.md (PRs view), ADR 0210's amendment, CHANGELOG. Out of scope, filed as CSP-880: the header's `▲ N` and harness chips still count rows. `checks.sh` and `just check` (5705 passed) green.
<!-- SECTION:FINAL_SUMMARY:END -->
