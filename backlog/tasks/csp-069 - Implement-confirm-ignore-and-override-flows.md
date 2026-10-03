---
id: CSP-069
title: 'Implement confirm, ignore, and override flows'
status: Done
assignee: []
created_date: '2026-05-16 04:29'
labels:
  - p5
milestone: m-6
dependencies:
  - CSP-068
ordinal: 65000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add mutation flows that mark a discovered candidate as
  confirmed declared evidence, record ignored candidates with optional
  reasons, and record explicit overrides that point to the replacing
  declared link while keeping original evidence visible.
- Tests: CLI integration and resolver tests for confirmed mux links,
  ignored noisy candidates, overridden links, local-vs-global state,
  optional reasons, and detailed graph JSON preserving all candidate
  evidence.
- Manual checks: confirm one discovered session↔mux candidate, ignore
  a competing candidate, and inspect `candidate_links`,
  `resolved_relationships`, and diagnostics.
- Blockers: `CSP-068`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`conspectus declared confirm` and `declared ignore`
share a `run_confirm_or_ignore` helper that runs discovery, looks
up the candidate by id in `snapshot.candidate_links`, maps both
endpoints back to `DeclaredEndpoint` via a new
`declared_endpoint_from_node_id` helper, and writes a declared
link with state Active or Ignored (carrying the supplied
`--reason` for ignore). `declared override` uses a new
`load_declared_link_by_id` helper to read the existing declaration
from the same store, mutates state to Overridden plus
`overridden_by` + optional reason, and writes it back. Six new
CLI smoke tests cover confirm, ignore with reason, unknown
candidate id, override of an existing link, override missing id,
and a graph-JSON assertion that the discovered candidate stays
visible alongside the new local-declared one.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P5-010`
