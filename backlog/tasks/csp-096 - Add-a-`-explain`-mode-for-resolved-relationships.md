---
id: CSP-096
title: Add a `--explain` mode for resolved relationships
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-obs
milestone: m-11
dependencies:
  - CSP-085
ordinal: 113000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: surface why the resolver picked a given winning candidate
  (provenance tier, recency, state, conflict diagnostics). Both for the
  JSON output and `session` table cells with the `*` ambiguity marker.
- Tests: snapshot tests for ambiguous mux and PR fixtures.
- Blockers: `CSP-085` is friendlier to do first because the
  explanation depends on a stable scoring shape.
- Related: ADR 0059 (Accepted) frames this as the immediate work and
  defers the rules-engine question behind it; review and accept/reject
  via `CSP-404` before scoping `--explain` implementation.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `resolve::explain_resolved_relationships`, which
annotates each `ResolvedRelationship` with the selected candidate's
score axes, competing candidates' score axes, and the first axis
that differs from the nearest competitor. `graph --explain` emits
those annotations in JSON, while default graph JSON remains compact.
`node show` annotates before rendering and prints the same score
breakdown for relationships touching the node. The first scoring
surface covers generic precedence/confidence, `linked_to_mux`, and
`branch_has_forge_pr`; future comparator-specific axes can extend
the same carrier.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-OBS-004`
