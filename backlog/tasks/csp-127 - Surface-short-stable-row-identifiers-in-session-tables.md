---
id: CSP-127
title: 'Surface short, stable row identifiers in session tables'
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 119000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Every `conspectus session` projection now emits a leftmost
`ID` column carrying a short, content-addressed prefix derived from
the row's primary `NodeId`. The hash is FNV-1a 64-bit over the
`Display` form of the NodeId (`pub fn node_short_id` in
`src/output/table.rs`), exposed so `node show` (CSP-130) can
resolve a pasted id back to a node. Prefix length is the minimum
needed for uniqueness within the rendered snapshot, floored at six
hex chars. The union projection's existing `ID` header (which held
the harness label) was renamed to `LABEL` to free the `ID` slot for
the new short id. JSON output is unchanged. New unit tests pin the
hash determinism, prefix-growth-on-collision, and per-projection
header changes; the atelier-delegation and declared-snapshots
fixtures gained per-snapshot `state_scope` path normalization so
the rendered short id stays stable across runs.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-002`
