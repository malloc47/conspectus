---
id: CSP-857
title: 'TUI: dim a mux live mark when every agent behind it is quiet'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 22:42'
updated_date: '2026-11-14 14:39'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: enhancement
ordinal: 942000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0209 draws a mux's rolled-up live mark never dimmed, because a mux row carries only per-state counts (`LiveCounts`) and no confidence. A mux whose only working agent has gone quiet therefore shows a bright `»` while that agent's own row shows it dimmed. Dimming the mux mark when every agent in its dominant state is quiet needs a quiet count per state on the mux row. CSP-823 and CSP-824 left this open; it changes ADR 0209 decision 3, so it needs the operator's decision.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Operator-approved 2026-11-13: a mux whose agents in its dominant state are all quiet draws its mark dimmed, and one with a fresh agent in that state draws it bright
- [x] #2 The mux row model carries a quiet count per state, filled by the same pass that fills the counts, and the live overlay refreshes it in place when an agent goes quiet
- [x] #3 The detail pane's mark for a mux reads the same sign as its row
- [x] #4 docs/tui-columns.md and an ADR amending ADR 0209 decision 3 describe the rule; tests cover both cases and the in-place refresh; the showcase has a mux that exercises the dimmed mark
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Model: a quiet-count-by-mux derivation beside live_counts_by_mux, from the same agent list (one shared helper), counting agents whose effective confidence is quiet per state.
2. rows/live.rs: MuxLive gains quiet: LiveCounts; LiveValues::at fills it in the pass that fills counts; LiveSign::of_mux quiet = every agent of the dominant state is quiet.
3. Tests first: of_mux both cases and the other states; LiveValues filling; overlay poll turning a fresh working agent quiet refreshes the row in place; the detail pane shares the sign.
4. Showcase: data that exercises it, frames before/after at 170 and 70 columns, just tui-columns.
5. ADR (amends 0209 decision 3), docs/tui-columns.md prose and column table, CHANGELOG, operations.md.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision 2026-11-13: approved. Rule as decided: dim when every agent in the mux's dominant state is quiet, bright while any agent in that state is fresh (only that state counts; recorded in ADR 0233 with the consequence that a quiet working agent beside a fresh starting one still dims). Showcase: `showcase-new-pane` (pane %3 of `project`) now has had no news for eleven minutes, so `project`'s `»` is dimmed; the fixture was patched by hand in the two values the scenario change moves, since regenerating it also rewrites every freshness_epoch. Frames at 150, 170, and 70 columns: only the Mux view changes, and only by SGR 2 on `project`'s `»` (plain text identical). README screenshots (docs/images) were not re-rendered (termshot is not available here); the Mux screenshot's `project` mark would dim on the next `just screenshots`. Validation: mark, row, model, and consistency tests (mutation-checked: they fail without the rule); `just check` green (5744 passed).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A mux's rolled-up live mark now draws dimmed when every agent in its dominant state is quiet and bright while any agent in that state is fresh (ADR 0233, amending ADR 0209 decision 3 and following 0213 decision 7's detail pane). `MuxLive` carries `quiet: LiveCounts`, filled in `LiveValues::at` by the same pass as the counts from `GraphSnapshot::quiet_counts_by_mux`, which shares its agent list with `live_counts_by_mux`; `LiveSign::of_mux` is the one place the rule lives, so the Mux view, a stopped mux, a mux pin, and the detail pane's `agents` mark agree. The overlay's one-second check already notices an agent crossing its quiet window, so the rows refill in place. Showcase data now exercises it (`project`'s two working agents are both quiet); docs/tui-columns.md, operations.md, dev-scenarios.md, and CHANGELOG describe it. Verified with unit tests for the model count, the mark rule (both cases, other states, color and muted look), the row pass and in-place refill, a consistency test that drives `poll_live` and reads the drawn cell, a mutation check, before/after frames, and `just check` (5744 passed).
<!-- SECTION:FINAL_SUMMARY:END -->
