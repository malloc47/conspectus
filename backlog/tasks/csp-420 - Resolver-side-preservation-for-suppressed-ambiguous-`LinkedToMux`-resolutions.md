---
id: CSP-420
title: Resolver-side preservation for suppressed ambiguous `LinkedToMux` resolutions
status: Done
assignee: []
created_date: '2026-06-17 19:04'
labels:
  - h-ui
milestone: m-11
dependencies: []
ordinal: 360000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0077 records the shape decision —
`ResolvedRelationship.selected_link_id` becomes
`Option<String>`; `None` marks the resolver-can't-pick case.
`suppress_ambiguous_cwd_mux_links` mutates the matched slot
in place: it clears `selected_link_id` and pushes the
would-have-been winner id into `competing_link_ids` so the
candidate set stays complete. The SQLite schema drops the
NOT NULL on `resolved_relationships.selected_link_id`; the
serde derive picks up `skip_serializing_if = "Option::is_none"`
so existing winners serialize unchanged.
Both ad-hoc fallbacks retire: `build_relationship_group` in
`src/tui/explorer.rs` drops the CSP-421 candidate-fan-out
inference and reads `selected_link_id.is_none()` directly to
mark a slot ambiguous; `mux_candidates_for_session` in
`src/tui/rows/sessions.rs` drops the CSP-422 candidate
fallback and walks the slot — `Some` returns the winner,
`None` returns every link in `competing_link_ids` so
`MuxStateKey::Ambiguous` still fires. The
`by_source_relation` index and `pick_preferred` import are
no longer needed; both deleted.
Tests: the resolver suppression suite (three tests) now
asserts the slot survives with `selected_link_id = None`
and the candidate set rolls into `competing_link_ids`. The
explorer regression
`linked_to_mux_suppressed_slot_surfaces_as_no_winner_ambiguous_group`
replaces the prior CSP-421 fallback test, asserting the
group is marked ambiguous, every row drops into the Other
zone, and no `Resolves` row exists. `testing_replay.rs`
asserts compare `selected_link_id.as_deref()` against
`Some(...)`. The showcase fixture regenerated to include
the preserved suppressed slots.
Followup: `Diagnostic::Conflict.competing_link_ids` is still
`vec![]` for suppression diagnostics — see the ADR's open
question; out of scope here.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-UI-006`
