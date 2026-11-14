---
id: CSP-861
title: 'TUI: count a mux''s counted agents in the Mux view''s N/M sessions'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 02:51'
updated_date: '2026-11-14 01:17'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 946000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: under a narrowing filter the Mux view's header reads `N/M sessions` from the sessions the listed muxes attach by a resolved link (`attached_count`). A mux listed for the agents its mark counts (an ambiguous session whose preferred mux it is, an agent in a pane no session matches) adds none, so `agent:waiting` showing only the showcase's `ambiguous` mux reads `0/11 sessions` beside a mux whose mark says two agents wait (ADR 0215, consequence 2). Decide whether N should count the sessions a mux's live counts hold, or say why it does not.
- Tests: header count under `agent:waiting` in the Mux view over a world with an ambiguous waiting session; the count under the other filters unchanged.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Under agent:waiting the Mux view's N counts the waiting sessions a listed mux's mark counts that no link attaches (the showcase reads 2/11 sessions, was 0/11)
- [x] #2 Agents in panes no session matches add nothing to N, and each session counts once
- [x] #3 N is unchanged under filters with a session predicate other than the agent state, and every unfiltered frame is unchanged
- [x] #4 docs/tui-columns.md states what N counts in the Mux view; ADR 0215 records the change
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Decision (the brief: N/M against the agents the marks count): under a narrowing filter N counts the sessions the listed muxes hold, each once: the agents each lists and, while the agent state is the only session predicate, the sessions its mark counts in a kept state that no link attaches to any mux. Pane agents are not sessions (not in M) and add nothing.
2. Failing test in src/tui/ui_tests.rs: a world with two cwd-ambiguous sessions counted on one mux (one waiting, one working), an attached working session, a waiting pane agent, and an idle session; header N under waiting, working, both, harness, harness+waiting.
3. LiveStates::sessions accessor; MuxSessionRow.held_sessions filled by build_mux_tree; the header sums it.
4. Docs: tui-columns.md Mux view prose, ADR 0215 amendment, CHANGELOG.
5. Frames before/after (frames.sh; --snapshot at 170/100/70/50 in the Mux view under each state filter); checks.sh, just-check.sh; commit; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Validation: mux_view_header_counts_the_sessions_a_listed_mux_s_mark_counts failed before (0/4 under agent:waiting) and passes after. First test world was wrong: two equal non-cwd candidates still select a link (the session is attached, Ambiguous indicator); only cwd-evidence links to a mux with several sessions are left unselected, as in the showcase. frames.sh (9 views) and --snapshot of Sessions, Mux, PRs at 170/100/70/50 unchanged; Mux view under --state waiting/working: header 0/11 -> 2/11 at every width, rows unchanged; ready and error unchanged. checks.sh clean; just check passed (5694 passed, 1 skipped).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Decided N should count them. Under a narrowing filter the Mux view's `N/M sessions` now counts the sessions the listed muxes hold, each once: the agents each mux lists and, while the agent state is the only session predicate (when ADR 0215 lists a mux for what its mark counts), the sessions its mark counts in a state the filter keeps that no link attaches to any mux. Agents in panes no session matches are not among the M sessions and add nothing; filters with another session predicate count as before. `MuxSessionRow.held_sessions` (filled by `build_mux_tree` at build time, from a new `LiveStates::sessions` accessor) replaces `attached_count` in the header's sum. docs/tui-columns.md (Mux view) states the rule; ADR 0215 gains an amendment replacing consequence 2; the agent-state CHANGELOG entry mentions it.

Verification: new `mux_view_header_counts_the_sessions_a_listed_mux_s_mark_counts` (waiting 1/4, working 2/4, both 3/4, harness 1/4, harness+waiting 0/4) failed before (0/4) and passes after. frames.sh and `tui --snapshot` at 170/100/70/50 compared before/after. `checks.sh` clean; `just check` passed (5694 tests, 1 skipped).

**Regressions:** none found.
**Consistency wins:** in the showcase Mux view, `--state waiting` and `--state working` read `2/11 sessions` (were `0/11`) at all four widths, matching the `ambiguous` mux's mark; rows, `--state ready`/`error`, and every unfiltered frame are unchanged.
<!-- SECTION:FINAL_SUMMARY:END -->
