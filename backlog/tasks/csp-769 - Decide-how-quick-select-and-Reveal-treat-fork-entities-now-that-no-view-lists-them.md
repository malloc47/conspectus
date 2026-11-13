---
id: CSP-769
title: >-
  Decide how quick select and Reveal treat fork entities now that no view lists
  them
status: Done
assignee:
  - '@claude'
created_date: '2026-10-28 05:01'
updated_date: '2026-10-31 01:38'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
priority: low
type: chore
ordinal: 855000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Retiring the Forks view (CSP-711) left `actions::natural_view` with no home for a live fork. Fork nodes are still in the quick-select corpus (`search_entities.rs`) and can be pinned (`PinKind::Fork`), so a fork that quick select cannot reach along a relationship path now reports "in Sessions view" and then fails on Enter with "isn't listed in the Sessions view". The Pinned row menu also still offers "Reveal in its view" for a fork pin, which fails the same way. Options: leave unreachable forks out of the corpus, give the open plan a "not listed" outcome, hide the Reveal entry for kinds no view lists, or give forks a home (for example as rows in the Sessions graph). The operator picks.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Quick select never promises a view that does not list the entity it found
- [x] #2 The Pinned row menu offers Reveal only when the entity is listed in some view
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Decision (consistency-first, ADR 0151; the task leaves it to the operator, so the default is recorded for review): one rule for both surfaces, "a gesture is offered or promised only where it lands". Quick select gets a `not listed` outcome (option 2) and the Pinned row menu hides Reveal when its view would not list the entity (option 3, decided per entity rather than per kind, since a repo with no sessions is as unlisted as a fork). Not option 1 (whether a fork is reachable depends on the focus, so results would come and go) nor option 4 (a new home for forks is a product change, not a cleanup).
1. `natural_view` answers `None` for the kinds no view lists (forks, branches, processes), so nothing names a view for them.
2. `App::revealed_tree(view)` builds `view`'s tree as revealing it would show it (its saved grouping and filter, History's pinned records, the flat-sessions sort); `App::reveal_view(node, record)` says which view a reveal lands in, if any. Reveal's own row predicate matches pin rows too (`row_matches`), which it missed.
3. Quick select: `OpenPlan::Unlisted` (word `not listed`) when a result is not in the current view, not on a path, and not listed by its view; computed from a per-view listing built once when the reach widens (stored with the entity index), and again at Enter. Enter on it says why and stays put.
4. The row menu offers **Reveal in its view** only when `reveal_view` lands elsewhere.
5. Tests: every entity of the all-kinds world: the word's promise is what Enter does (view switched and row selected, or a refusal); every pin's menu offers Reveal exactly when RevealSlot succeeds. Docs, changelog, ADR status notes (0174/0175 family) as needed, operator-review row.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decision recorded in ADR 0198 and operator-review (the task reserved it for the operator; consistency-first default shipped). Verified: `the_outcome_word_promises_a_view_only_where_enter_finds_the_row` (every entity of the all-kinds world from Sessions, Mux, and Pinned: the drawn word equals Enter's plan, a named view is where Enter lands, `not listed` stays put with "isn't listed"), `reveal_is_offered_only_where_it_lands` (every pin: Reveal offered iff RevealSlot lands elsewhere; offered for mux-live, session-live, repo-live, the three PRs; withheld for fork-live, branch pins, planned/missing pins, and targets their view does not list), listing tests (the predicted view equals where a real reveal lands for every node from three views; a pin reveals in Pinned, which failed with the old row-id lookup: verified by reverting it). Frames /tmp/tuifw-baseline/frames-769-{before,after} (100x30, 60x20): only quick-select words changed (`in Sessions view` -> `not listed` for the fork `alpha`, its fork checkout, single-worktree checkouts, a relative-path repo); pin menus unchanged in the showcase. The History row-menu snapshot lost Reveal for a gone mux whose Mux view lists no ghost (its reveal failed). Full isolated suite 4809 passed, 0 failed, 1 ignored; fmt, both clippy runs, doc, examples, diff --check pass.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Decided by the consistency-first rule (the task left it to the operator; recorded in ADR 0198 and operator-review): a gesture is promised or offered only where it lands, decided per entity on the tree the view would show after a reveal switches to it.

- Quick select's wider reach gets a fourth outcome, `not listed`, for a result nothing focused leads to and no view lists; `Enter` there says why and stays put. `in <View> view` now appears only when that view lists the entity. The listing is built once when the reach widens (`App::listing`), so drawing builds no tree.
- A pin's menu offers **Reveal in its view** only when `App::slot_reveal_view` lands in another view.
- `natural_view` answers `None` for forks, branches, and processes (a reveal says "isn't listed in any view").
- Found and fixed on the way: a reveal looked rows up by row id, so a pin found by quick select from another view (`in Pinned view`) failed; it now finds the row by the entity it stands for, as `here` does.
- Not chosen: dropping unreachable forks from the index, giving forks a home.
- Tests: `the_outcome_word_promises_a_view_only_where_enter_finds_the_row`, `reveal_is_offered_only_where_it_lands`, `listing_tests.rs` (predicted view equals a real reveal for every node; fork listed nowhere; pin reveals in Pinned), `natural_views_follow_the_kind…` (fork and branch `None`); the History row-menu test and snapshot updated (Reveal withheld where its reveal failed).
- Docs: ADR 0198 (amends 0149 and 0144), operations guide, design, changelog.

**Regressions:**
- A pin whose entity its view does not list as set up loses the Reveal entry it used to show (and fail on), for example a gone mux where the Mux view lists no ghost; its reveal keys still explain themselves.

**Consistency wins:**
- Quick select reads `not listed` instead of a broken `in Sessions view` for a fork, its fork checkout, a checkout folded into its repo's row, and a repo no view lists (frames `qs-*`).
- A fork's or branch's pin no longer offers a Reveal that fails.
- A pin found by quick select from another view opens in the Pinned view.

Checked with `conspectus tui --snapshot` at 100x30 and 60x20 (`/tmp/tuifw-baseline/frames-769-*`); the showcase pin menus are byte-identical. Full isolated suite 4809 passed, 0 failed, 1 ignored; fmt, both clippy runs, doc, examples, and `git diff --check` pass.
<!-- SECTION:FINAL_SUMMARY:END -->
