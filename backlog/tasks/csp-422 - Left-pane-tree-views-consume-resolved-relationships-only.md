---
id: CSP-422
title: Left-pane tree views consume resolved relationships only
status: Done
assignee: []
created_date: '2026-06-17 23:43'
labels:
  - h-ui
milestone: m-11
dependencies:
  - CSP-419
ordinal: 362000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: today the row builders for the sessions, mux, prs,
  and forks views pull from `candidate_links` directly with no
  filter to the resolver's chosen winners. ADR 0074 + ADR 0075
  set up the detail-pane invariant that the validated zone is
  the resolver-pick zone and the Other zone is everything else;
  the tree views violate the symmetric invariant — a row in the
  tree can be derived from a link the detail pane would route
  to Other. Drive the row builders through
  `resolved_relationships` (joining back to `candidate_links`
  on `selected_link_id` for the link payload) so what shows up
  in the tree matches what the detail pane calls validated.
  Specific call sites to touch:
    - `src/tui/rows/sessions.rs:566` `mux_candidates_for_session`
      — currently picks the highest-provenance candidate per
      mux target via `pick_preferred`; switch to "use the
      resolver's winner for the `LinkedToMux` slot, fall back
      to nothing." The `AgentSessionMuxCandidate` row type
      loses its fan-out semantics on resolver-blessed slots
      and only fires when the resolver explicitly couldn't
      pick (`suppress_ambiguous_cwd_mux_links`, CSP-420).
    - `src/tui/rows/sessions.rs:604` `workspace_for_session` —
      first-wins over candidate links today; switch to the
      resolver's `AssociatedWith` winner.
    - `src/tui/rows/mux.rs:657` SQL — `FROM candidate_links cl`
      with no resolved-only filter; add a join to
      `resolved_relationships` so the "attached agents" list
      only shows resolver-blessed attachments.
    - `src/tui/rows/prs.rs`, `src/tui/rows/forks.rs` — similar
      SQL pattern; review and switch.
  Keep one explicit candidate-aware surface for the resolver's
  legitimately-can't-pick cases (the detail-pane Other zone
  plus the `AgentSessionMuxCandidate` row when
  `suppress_ambiguous_cwd_mux_links` fires) so operators
  investigating ambiguity still have a path.
- Impact assessment (scanned `~/src` 2026-06-17 against the
  CSP-419 commit): 305 active candidate links total, only 8
  are non-winners (2.6%). Breakdown:
    - `linked_to_mux`: 3 non-winners (2 real competitors +
      1 unresolved-endpoint variant) — these are the most
      operator-visible (false-positive "attached agents" on
      mux rows).
    - `parent_session`: 4 non-winners (4 distinct children
      each with a duplicate candidate pointing at the same
      parent; resolver tie-broke). Not legitimate siblings —
      the resolver already emits each
      `(child, parent_session)` slot independently because
      the slot key is per-source, so distinct children all
      win their own slots.
    - `process_candidates_session`: 1 non-winner (unresolved-
      endpoint variant).
  Most of the change is *removals* (cleaning up false-positive
  rows) rather than hiding useful evidence; the impact at the
  operator's typical scale is small and lopsided toward
  clarity.
- Cardinality note (preserve in implementation): the resolver
  keys slots by `(source, relation, target_key)` with
  `target_key = Some(target)` for the `multi_target_relation`
  set (`src/resolve/mod.rs:571`:
  `AssociatedWith | WorkspaceContainsRepo |
  MuxContainsProcess | ProcessIdentifiesSession |
  ProcessCandidatesSession`). For everything else, candidates
  with the same source compete for one slot. **1:N
  relationships from the target's perspective still work
  correctly** under this filter because each row on the "many"
  side is the *source* of its own slot — a mux with three
  attached agent sessions has three independent
  `(session, linked_to_mux)` slots, each with its own winner;
  filtering the mux view through `resolved_relationships`
  surfaces all three. Same logic for "parent has many
  children": each child is the source of its own
  `parent_session` slot. The filter only hides candidates that
  *lost their own per-source slot*, which are by definition
  duplicates or ambiguity cases. A separate audit may revisit
  `multi_target_relation` completeness (e.g. should
  `BranchHasForgePr` move into the set?) but it does not block
  this story.
- Tests: row-builder unit tests that pin "non-winner candidate
  links are not surfaced in the tree" across the
  `mux_candidates_for_session` / mux-view SQL / PR / fork paths.
  Snapshot updates for the showcase scenario where the
  `AgentSessionMuxCandidate` fan-out was previously emitted
  from non-conflict candidates. Resolver-side coverage that
  `suppress_ambiguous_cwd_mux_links` still produces the
  candidate fan-out in the tree (preserves CSP-421's signal).
- Open questions: whether the mux view's "attached agents"
  column should fall back to candidate links when the resolver
  didn't pick (preserves the historical UI signal) or simply
  hide attachments (matches the detail-pane invariant exactly).
  Recommend the latter for consistency.
- Blockers: CSP-419 (so the detail-pane half of the invariant
  is in place); ideally lands alongside CSP-420 so the
  resolver-side and renderer-side stories agree on what
  "candidate fan-out" means.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`mux_candidates_for_session` and `workspace_for_session`
in `src/tui/rows/sessions.rs` now read winners from
`snapshot.resolved_relationships` instead of grouping raw
candidates. The mux view's `fetch_attached_agents` SQL gains
a `JOIN resolved_relationships rr ON rr.selected_link_id =
cl.link_id AND rr.relation = 'linked_to_mux'`, dropping the
false-positive attachments under non-winning cwd evidence.
Same pattern lands in `src/tui/rows/prs.rs` for
`BranchHasForgePr` and in `src/tui/rows/forks.rs` for
`ChildSession` (INNER JOIN) and `ParentSession` (LEFT JOIN +
`OR target_kind = 'unresolved'` so unresolved-endpoint labels
survive — the explicit candidate-aware surface CSP-422 calls
out for resolver-can't-pick cases). The
`mux_candidates_for_session` body retains an CSP-421-style
candidate fan-out fallback (no resolver entries + ≥2 distinct
candidate targets) so `suppress_ambiguous_cwd_mux_links` keeps
surfacing genuine ambiguity until CSP-420 lands the resolver-
side preservation. Tests: per-call-site regression that a
non-winner candidate no longer surfaces in the tree
(`mux_view_drops_non_winner_linked_to_mux_candidate`); the
existing `invariant_ambiguous_mux_session_renders_as_leaf_after_adr_0071`
scenario updated to use 2+ sessions so it exercises the real
suppression path; the false-positive
`two_mux_links_yield_ambiguous_and_expandable_with_candidate_children`
rewritten as
`two_mux_links_with_distinct_provenance_resolve_to_one_attached_mux`
pinning the cleaned-up behavior.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-UI-008`
