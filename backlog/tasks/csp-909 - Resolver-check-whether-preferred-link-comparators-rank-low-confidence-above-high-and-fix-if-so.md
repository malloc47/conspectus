---
id: CSP-909
title: >-
  Resolver: check whether preferred-link comparators rank low confidence above
  high, and fix if so
status: Done
assignee:
  - '@claude'
created_date: '2026-11-25 18:58'
updated_date: '2026-11-25 19:19'
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
- [x] #1 The task notes state, for each comparator that ranks by confidence, whether it reaches the confidence tie-break in practice and which direction its docs or ADRs intend
- [x] #2 A test pins the intended direction for `pick_preferred` and for each other comparator the finding settles, using links of equal provenance and different confidence
- [x] #3 If the order is inverted, each inverted comparator is fixed, every resolved relationship and snapshot that changes is reviewed and listed in the task notes, and an ADR records the direction if it was a decision
- [x] #4 `just check` passes
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Read every comparator that ranks by confidence and the producers of the links they rank (what ties, and when).
2. Instrument each comparator's confidence key (it runs only when every earlier key tied) and run the whole suite, to learn whether any real comparison is decided by confidence. Remove the probe afterward.
3. Pin the intended direction with a test per place, using links that tie on everything ahead of confidence and ids that sort against strength. Confirm they fail on the old code.
4. Fix at the type: give `Confidence` a hand-written `Ord` by strength rather than editing nine call sites or reordering the archived variants; pin the wire and archive form.
5. Record the direction in an ADR, add the index row and a changelog entry, and run `just check`.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Finding, per place (criterion 1). Intended direction everywhere: the strongest link first. Evidence: the explorer's comment calls its first link for a neighbor "its strongest candidate", the table output reads `H`/`M`/`L`, and ADRs 0035 and 0041 name the keys without a direction. Every place sorted `right.confidence.cmp(&left.confidence)`, the right idiom for numeric ranks, but `Confidence` derived `Ord` with `High` first (smallest), so each put `Low` first at a tie.
Reached in practice: no place, in any of the 6037 tests. A temporary probe on each confidence key (it only runs when every earlier key tied) logged nothing across the suite, fixture corpus and end-to-end scenarios included; the same probe fired at all eight sites once the new tests ran, so it worked. By inspection of the producers, confidence moves with what decides first: session-mux links from an exact cwd, a process, or a hook are `StrongDiscovered`/High and from a cwd prefix `Discovered`/Medium (`src/discovery/cross_link.rs`), so tier and evidence rank separate them; branch-PR links are all High (`forge/github.rs`), so the key never differs; process-identity links of one relation are all High. `pick_preferred`, `preferred_mux_for`, `preferred_session_for_mux`, `compare_generic`, and the explorer's sort have no key between provenance and confidence, so they would be first to hit a producer that ties. Not checked: whether Atelier lineage links (confidence varies by harness capability) ever compete for one slot.
Fix: `Confidence` has a hand-written `Ord` (`High > Medium > Low`), `src/model/link.rs`. One change corrects all nine places because each already reads "descending is strongest first". Variants keep their declaration order, serde names, and archived discriminants (0, 1, 2), pinned by `confidence_keeps_its_wire_and_archive_form`. ADR 0257 records the direction.
Changed resolved relationships or snapshots (criterion 3): none. The full suite passes with no snapshot or fixture edited.
Tests: 12 new, in `model/model_tests.rs` (order and archive form), `model/graph_tests.rs` (`pick_preferred`, `preferred_mux_for`, provenance outranking), `tui/actions_tests.rs` (the session `v` opens), `tui/explorer_tests.rs` (group order), `resolve/mod_tests.rs` (the four comparators directly, end-to-end selection, and evidence outranking confidence). Eight of them failed on the old code and pass now; two guard that provenance and evidence still come first.
Validation: `just check` green after the last change (6049 passed, 1 skipped; fmt, clippy, rustdoc -D warnings, diff check; tuikit-features skipped, nothing under crates/tuikit changed). Not run locally: CI's `cargo test --workspace`.

Criterion 4 ("if the order is intended") was the other branch of the investigation and did not apply, so it was removed rather than checked.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The suspected bug was real but latent. `Confidence` derived `Ord` with `High` smallest, and nine places that pick the preferred link sorted it descending like a number, so among links that tied on every earlier key the weakest won: `pick_preferred`, `preferred_mux_for`, the session `v` opens, four resolver comparators, and the explorer's group order. Nothing reached it: a probe on every confidence key logged no decided comparison across the 6037-test suite, because producers keep confidence in step with provenance and match evidence. The fix is one hand-written `Ord` (`High > Medium > Low`) in `src/model/link.rs`, which corrects every place at once and leaves variant order, serde names, and archived discriminants (0, 1, 2) as they were. ADR 0257 records the direction; the ADR index and changelog are updated. No resolved relationship or snapshot changed.
Verified with 12 new tests, one per place plus the order, the archive form, and provenance and evidence still outranking confidence (8 fail on the old code), and `just check` (6049 passed, 1 skipped; fmt, clippy, rustdoc, diff check). Not run locally: CI's `cargo test --workspace`.
<!-- SECTION:FINAL_SUMMARY:END -->
