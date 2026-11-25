---
id: CSP-909
title: >-
  Resolver: check whether preferred-link comparators rank low confidence above
  high, and fix if so
status: To Do
assignee: []
created_date: '2026-11-25 18:58'
updated_date: '2026-11-25 18:58'
labels:
  - resolver
dependencies: []
references:
  - src/model/link.rs
  - src/model/graph.rs
  - src/tui/actions.rs
  - src/resolve/mod.rs
  - src/tui/explorer.rs
priority: medium
type: bug
ordinal: 984000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every comparator that picks the preferred link sorts confidence as `right.confidence.cmp(&left.confidence)`, and `Confidence` (`src/model/link.rs`) derives `Ord` with the variants declared `High, Medium, Low`. If that reading is right, then among links that tie on the keys ahead of it the lowest-confidence one sorts first and wins. No test pins either direction.

It surfaced while building CSP-908: a test gave a second session's `linked_to_mux` link `Confidence::High` to make it the one `v` opens on a mux, and the existing `Medium` link kept winning. The test was changed to rank by provenance instead, and nothing else was touched, because a change here moves resolved relationships and the snapshots built from them.

The sites found by searching for `confidence.cmp(` are `pick_preferred` (`src/model/link.rs`, used by the CLI output for agents, muxes, and forks), `GraphSnapshot::preferred_mux_for` (`src/model/graph.rs`, which decides a session's attach target and row glyph), `preferred_session_for_mux` (`src/tui/actions.rs`, the session `v` opens on a mux), four comparators in `src/resolve/mod.rs` (around lines 702, 721, 785, and 913), and one sort in `src/tui/explorer.rs` (around line 1796). Only `pick_preferred`, `preferred_mux_for`, and the first of the resolver's comparators (`compare_generic`) were read closely: in those, provenance precedence is the first key, so only ties on it are affected. The process-identity comparator ranks evidence first, and the rest were not read.

What is not known: whether the direction is intended for some relation, how many of these sites actually reach the confidence tie-break, and what a real graph would change if it were reversed. The task is to find out, and fix it if it is real.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The task notes state, for each comparator that ranks by confidence, whether it reaches the confidence tie-break in practice and which direction its docs or ADRs intend
- [ ] #2 A test pins the intended direction for `pick_preferred` and for each other comparator the finding settles, using links of equal provenance and different confidence
- [ ] #3 If the order is inverted, each inverted comparator is fixed, every resolved relationship and snapshot that changes is reviewed and listed in the task notes, and an ADR records the direction if it was a decision
- [ ] #4 If the order is intended, the comparators or `Confidence` say why in a comment, so the question is not reopened, and the tests above pin it
- [ ] #5 `just check` passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
