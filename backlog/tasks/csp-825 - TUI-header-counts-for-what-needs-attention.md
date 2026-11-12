---
id: CSP-825
title: 'TUI: header counts for what needs attention'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 23:05'
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
ordinal: 911000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
One glance at the header should answer "does anything need me?". What the chip counts (waiting only, or waiting plus ready-unseen plus working), whether it follows the filter, and where it ranks against freshness, counts, and the warning mark are open.

Scope: Per the operator's decision: the chip, its rank and give-way order (ADR 0190), and a key or click that filters to it.

TUI changes: The header gains a chip.

Tests and checks: Header snapshots at several widths; chip width rule tests (ADR 0157).

Blockers: Operator decision 3 and the cell story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The header shows `! N`, the waiting mark and the number in the waiting mark style (the same chip as a group header count), when N > 0 agents anywhere in the graph wait on the operator: every session with live state and every agent in a pane no session matches, whatever the view lists or filters; nothing at zero
- [x] #2 The chip sits after the counts (and the opt-in harness chips) and before `▲ N`, set apart like the other chip sections; on a narrow screen it outlasts the stopped count and `▲ N` and gives way before the mux count and freshness, whole, ending the line with `…`, by the header give-way rule
- [x] #3 The count comes from the same live values that fill the rows, so a store change or time moving a state updates it in place with the rows (no rebuild, cursor kept)
- [x] #4 No key or click acts on it (the State filter story owns that)
- [x] #5 docs/tui-columns.md gains a header section with its parts, order, and give-way ranks and header frames at a wide and a narrow width (test-enforced); an ADR, the help legend, docs/operations.md, and CHANGELOG describe the chip; consistency tests check the whole-graph count in every view and under a filter, the header never cutting a part with the chip present, the give-way order, the style, and an in-place change
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. LiveValues (the one read of the live tables at a moment) also counts the agents anywhere that draw the waiting mark: node_live cells and pane_live entries, through LiveMark. annotate puts it on the RowTree beside the rows it fills, so builds and overlay refills keep it in step with the rows.
2. header_line pushes the shared waiting_count_chip as its own segment (rank between the mux count and the ambiguity chip; renumber the ranks), after the harness chips and before `▲ N`, with the chip-section gap. No key handler.
3. Tests: LiveValues/annotate count (sessions, panes, exited/stale/no evidence excluded), header unit tests (hidden at zero, position, style, rank at narrow widths), consistency: the same `! N` in every view and under a filter, never cuts a part with live state, give-way order, in-place change through the overlay.
4. Docs: ADR 0211 (amends 0078 and 0190 decision 5), tui-columns.md header section with pane=header frames (extend the frame test to take a pane), help legend line, operations.md, CHANGELOG, design.md, plan README.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 3: header chip '! N' with the waiting count over the whole graph, hidden at zero, ranked after freshness and counts and before the warning mark.

Chose: the chip sits after the opt-in harness chips (the two triage chips stay together), is not bold (the marks it counts are not, and it is the group headers' chip), and its number shares the mark's tone. The count is left on RowTree by rows::live::annotate from the same LiveValues as the rows, rather than recomputed per frame, so it never runs ahead of the marks. No key handler (CSP-826 owns filtering by state).
Validation: run-tests.sh exit 0 (5,443 passed, 0 failed, 1 ignored); checks.sh exit 0; snapshots at 170/100/70/50 for every view before and after (only the header line changed).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The header line shows `! N` when N agents anywhere wait on the operator (every session with live state and every pane-only agent, whatever the view lists or the filter hides), hidden at zero. It is the group headers' chip, a segment after the counts and the opt-in harness chips and before `▲ N`, ranked between the mux count and the triage chip (freshness 6, mux 5, waiting 4, `▲` 3, stopped 2, harness chips 1). The count is computed by the pass that fills the rows' live values and left on the row tree, so it changes in place with the marks. No key acts on it. ADR 0211; docs/tui-columns.md gains a header line section whose frames the reference test now renders (`pane=header`).

Commits: 4304a667 (ADR 0211), cf34df9e (frame test takes a pane), d59affa5 (feature and tests), facc4acf (docs).

**Regressions**
- At 50 columns the chip takes the place of `4 stopped` in Mux, Pinned, and History, and of `▲ N` in Sessions, PRs, and Search (by the decided rank).
- With agents waiting the header is eight cells longer, so the stopped count and `▲ N` leave sooner as the screen narrows.

**Consistency wins**
- One chip for the header and the group headers, in the marks' color.
- The same count in every view, unaffected by filters.
- It changes at the same moment as the rows' marks (one read of the live tables).

Visible changes: every view's header reads `... · 4 stopped  ·  ! 2  ·  ▲ 4` in the showcase at 70 columns and wider (`! 2` added), and `... · 4 mux  ·  ! 2 …` at 50.

Verified with run-tests.sh (5,443 passed, 0 failed), checks.sh, the header consistency tests at every width from 30 to 120, the reference frames, and snapshots at 170/100/70/50 before and after.
<!-- SECTION:FINAL_SUMMARY:END -->
