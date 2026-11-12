---
id: CSP-824
title: 'TUI: roll a mux''s agents up into one state'
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 21:30'
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
- [ ] #1 Pending the operator's decisions
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Operator decision (2026-11-12), see docs/plans/live-state/README.md section 7.1 item 2: mux rollup precedence waiting, then error, then ready, then working (compacting, starting, unknown, exited after). The mux row shows the rolled-up mark in the cell from CSP-823; nested agent rows keep their own. Group headers show the waiting count next to (N) when above zero.

From CSP-823 (ADR 0209): the mux row already draws its rolled-up mark in the live cell, by `LiveCounts::dominant` (the model's default precedence, which is the decided order), `·` when nothing is counted, and nested agent rows already carry their own marks. Left for this story: the waiting count next to `(N)` on group headers. Two open points CSP-823 recorded rather than decided: a mux's mark is never dimmed, because `LiveCounts` carries no confidence (dimming it when every agent behind the dominant state is quiet needs a quiet count per state on the mux row); and a pinned mux in Pinned keeps the word `live` rather than a rolled-up word.
<!-- SECTION:NOTES:END -->
