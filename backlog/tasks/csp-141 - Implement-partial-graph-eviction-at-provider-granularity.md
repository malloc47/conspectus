---
id: CSP-141
title: Implement partial graph eviction at provider granularity
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - p7
milestone: m-12
dependencies: []
ordinal: 381000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: five unit tests in `src/model/mod.rs::tests::
  evict_provider_*` cover multi-provider eviction,
  no-op on missing keys, link-only eviction (mutator shape),
  resolved-relationship clearing, and preservation of
  orphan nodes lacking provenance entries. Cold-rebuild
  equivalence is implicit in the `discover_local_with`
  wrapper around `discover_local_warm_with`: passing an
  empty prior reduces the new path to the prior cold-only
  behavior.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed as `GraphSnapshot::evict_provider(&str)` in
`src/model/mod.rs`. The primitive drops every node whose
`node_provenance` entry matches the provider, every
candidate link whose `source_metadata.adapter` matches, and
every matching provenance entry; it clears
`resolved_relationships` so the resolver re-derives against
the trimmed candidate set. Conservative on data without a
provenance entry (pre-instrumentation snapshots survive).
Used by `CSP-139` phase 3 inside `discover_local_warm_with`
to evict stale + always-evict slices before the warm-start
merge; ready for `CSP-142` to call on every provider tick.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P7-005`
