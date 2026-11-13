---
id: CSP-824
title: 'TUI: roll a mux''s agents up into one state'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 22:45'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies:
  - CSP-823
documentation:
  - docs/plans/live-state/tui-options.md
priority: low
type: enhancement
ordinal: 910000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A mux can host several agents (split panes, background sessions), each with its own state. The model offers counts and a default precedence, but which state the mux row shows, whether nested session rows carry their own, and whether group headers count attention are display choices.

Scope: Per the operator's decision: the precedence the mux row uses, how it composes with the mux's own age and attach state, and group-header counts next to `(N)` and the warning mark.

TUI changes: Mux rows and group headers change as decided.

Tests and checks: Snapshot frames for a mux with one, two, and mixed-state agents; consistency tests for rollups.

Blockers: Operator decision 2 and the cell story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 In the Sessions, PRs, and Search views a group header shows `! N` after its `(N)` count when N > 0 of the agent sessions beneath it wait on the operator, counting each session once however many rows beneath that header list it; a header with none waiting shows nothing new
- [x] #2 In the Mux view a group header counts the waiting agents of the muxes beneath it (each mux once, from its rolled-up counts, so a mux with one agent, pane-only agents, and nested agents count once), shown even where the header has no `(N)`
- [x] #3 The count is the `!` live mark and the number in the waiting mark style, in one column across the tree headers; it takes no cells when no header has one, never gives way on a narrow pane, and the `▲` warning follows it
- [x] #4 A live-state change refills the counts in place with the rows (no rebuild; cursor and folds kept)
- [x] #5 A mux row rolls its agents up by decision 2 (waiting, error, ready, working, then compacting, starting, unknown, exited) whatever the counts, never dimmed, nested agent rows keep their own marks; tests cover a mux with one agent, two in one state, and agents in mixed states
- [x] #6 The showcase has a group with two waiting sessions; docs/tui-columns.md (group header table, counts, regenerated frames), the help legend, docs/operations.md, CHANGELOG, and an ADR describe the count; consistency tests check every header count against the marks beneath it in every view, its column and width rules, and the text rules
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reuse CSP-823: LiveSign/LiveMark decide waiting; LiveCounts::dominant is the rollup (already decision 2). Add a LiveMark tone so the count chip and the marks share one style source.
2. GroupSummary gains `waiting`; compute_group_summaries counts distinct waiting session rows beneath each header (Sessions, PRs, Search), or in the Mux view the waiting counts of the distinct mux rows (running and stopped) beneath it, skipping nested session rows (their mux counts them).
3. Header schema gains a Waiting column (cap, never gives way) between Count and Warning; the cell is a StatusChip `! N` in the waiting tone, shown even without `(N)`.
4. Tests: unit tests for the summary rule (dedup, Mux view, Pinned buckets none), mux precedence over every pair of marks, a rendered Mux view with one/two/mixed agents; consistency tests (header count = distinct `!` rows beneath; Mux view = muxes' counts; column lines up; narrow widths keep it; in-place refill).
5. Showcase: the pinned project session waits (two waiting in `project`); regenerate the fixture's live tables.
6. Docs: ADR 0210, tui-columns.md (group header, counts, frames via just tui-columns), help legend line, operations.md, CHANGELOG; snapshots at 170/100/70/50 before and after.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 2: mux rollup precedence waiting, then error, then ready, then working (compacting, starting, unknown, exited after). The mux row shows the rolled-up mark in the cell from CSP-823; nested agent rows keep their own. Group headers show the waiting count next to (N) when above zero.

From CSP-823 (ADR 0209): the mux row already draws its rolled-up mark in the live cell, by `LiveCounts::dominant` (the model's default precedence, which is the decided order), `·` when nothing is counted, and nested agent rows already carry their own marks. Left for this story: the waiting count next to `(N)` on group headers. Two open points CSP-823 recorded rather than decided: a mux's mark is never dimmed, because `LiveCounts` carries no confidence (dimming it when every agent behind the dominant state is quiet needs a quiet count per state on the mux row); and a pinned mux in Pinned keeps the word `live` rather than a rolled-up word.

Verified the mux rollup: LiveCounts::dominant (via LiveSign::of_mux) already ranks by decision 2; added tests for every pair of states whatever the counts and a rendered Mux view with one, two same-state, and mixed agents. The mux's age and attach state are unchanged (its own age, the link cell); its mark is never dimmed (open point filed as CSP-857). The pinned mux word in Pinned stays `live` (filed as CSP-858). `(N)` still counts rows, so a session under two PRs of one checkout counts twice there and once in `! N` (filed as CSP-856).
Showcase: the pinned project session now waits on a question (was ready), so `project` counts two; the fixture's node_live table was regenerated and spliced in.
Validation: run-tests.sh exit 0 (5,436 passed, 0 failed, 1 ignored); checks.sh exit 0 (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check); just tuikit-features exit 0; frames before/after with shots.sh at 170/100/70/50 for every view.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Group headers now count the agents beneath them that wait on the operator: `! N` after `(N)`, one StatusChip in the waiting mark's tone, in a lined-up column that never gives way and takes no cells while no header has one. Sessions, PRs, and Search count their session rows whose mark is waiting, each session once; the Mux view counts every agent of its muxes (running or stopped) from the mux counts, so it can wait without `(N)`; Pinned and History count nothing. Counts are worked out from the rows' live values each frame, so a store change refills them in place. ADR 0210; the mux rollup precedence (decision 2) was already in place and is now tested for every pair of states.

Commits: bfd4df28 (ADR 0210), 13e16900 (feature and tests), 137f4d24 (showcase), ae0c0090 (docs).

**Regressions**
- On narrow panes, once any header shows a count every header reserves it (five cells for one digit), so header paths narrow and leave sooner: Sessions at 70 shows `atelier-repo-a+p…t` and at 50 `~/atel…epo-a`; the PRs checkout header loses its path at 50 and in the 100-column split.
- The help page's Agent state section has a wider mark column (its new `! N` row).
- Showcase data: the pinned project session reads `!` and, in Pinned, `waiting` instead of `›`/`ready`; Pinned frames no longer show a `ready` word.

**Consistency wins**
- Every view that groups shows where an agent waits from a folded header, as the same `!` in the same color as the rows.
- A Mux view header counts agents with no row of their own (a mux's one agent, pane-only agents) through the same counts as the mux's mark; nested agents are not counted twice.
- A session listed twice beneath a header counts once.
- Live mark styles come from one tone per mark, shared with the new chip.

Visible changes: Sessions `project (4)  ! 2  ▲`; PRs `conspectus/project (4)  ! 2  ▲` and `Draft (4)  ! 2  ▲`; Mux by repo `project ~/repos/project  ! 2`; Search by project `project (1)  ! 1  ▲`; the help legend row; the showcase row changes above.

Verified with run-tests.sh (5,436 passed, 0 failed), checks.sh, just tuikit-features, the tui-columns frame test, and snapshots at 170/100/70/50 before and after. Follow-ups: CSP-856, CSP-857, CSP-858.
<!-- SECTION:FINAL_SUMMARY:END -->
