---
id: CSP-856
title: 'TUI: count each session once in a group header (N)'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 22:42'
updated_date: '2026-11-14 01:05'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 941000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A group header's `(N)` counts the session rows beneath it, so a session listed twice beneath one header counts twice: in the PRs view, a checkout with two pull requests on one branch lists the branch's sessions under each PR. The waiting count beside it (`! N`, ADR 0210) counts each session once, so such a header can read `(2)  ! 1` over one waiting session. The two counts should agree on what one session is.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A session listed under two pull requests of one group counts once in that header's `(N)`, as in its waiting count
- [x] #2 Headers whose sessions are each listed once draw as before (the reference frames are unchanged)
- [x] #3 docs/tui-columns.md says `(N)` counts sessions, each once
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Failing tests in src/tui/ui/left_panel/waiting_tests.rs: a session listed twice beneath one header (two PRs of one checkout) counts once in (N), as in ! N; the header draws (1)  ! 1.
2. compute_group_summaries: count each session once per header by its node (primary_node), for (N) and the ambiguity warning alike.
3. Docs: docs/tui-columns.md Counts bullet says (N) counts sessions, each once; amendment note on ADR 0210 (its consequence said (N) counts rows); CHANGELOG Fixed entry.
4. frames.sh before/after and --snapshot at 170/100/70/50 in Sessions, PRs, Mux, Search; reference frames must not change.
5. checks.sh, just-check.sh; commit; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Validation: the two waiting_tests changes failed before the fix ((N) read 6 for 5 sessions; a session under two PRs read agents 2) and pass after. frames.sh and --snapshot at 170/100/70/50 (Sessions, workspace grouping, Mux, Mux by repo, PRs, PRs unfolded, PRs by repo) are byte-identical before and after; tui_columns_doc passes unchanged. checks.sh clean; just check passed (5693 passed, 1 skipped). Out of scope, filed as CSP-879: the header line's N in the PRs view also counts duplicate session rows.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A group header's `(N)` now counts the sessions beneath it, each once by node, as the waiting count `! N` does; the `▲` warning reads the same set. A session listed under two pull requests of one checkout counts once in its checkout's and repo's headers, so `(2)  ! 1` over one waiting session now reads `(1)  ! 1`. Changed `compute_group_summaries` (`src/tui/ui/left_panel.rs`); docs/tui-columns.md's Counts bullet says each session counts once; ADR 0210 gains an amendment replacing its consequence that `(N)` counts rows (index marked amended); CHANGELOG Fixed entry.

Verification: new test `a_session_listed_under_two_pull_requests_counts_once_in_its_header` (summaries and the drawn header `(1)  ! 1  ▲`) and the corrected assertion in `a_header_counts_the_sessions_beneath_it_that_wait_each_once` failed before and pass after. frames.sh (9 views) and `tui --snapshot` at 170, 100, 70, 50 in Sessions, Mux, and PRs (several groupings) are byte-identical before and after; the tui-columns reference frames are unchanged. `checks.sh` clean; `just check` passed (5693 tests, 1 skipped).

**Regressions:** none; no showcase frame changes (the showcase lists no session twice beneath a header).
**Consistency wins:** `(N)` and `! N` agree on what one session is; a header's count can no longer exceed the sessions it lists.

Filed CSP-879: the header line's `N/M sessions` in the PRs view still counts duplicate session rows under a filter.
<!-- SECTION:FINAL_SUMMARY:END -->
