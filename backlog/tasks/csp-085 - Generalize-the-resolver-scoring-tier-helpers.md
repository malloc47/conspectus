---
id: CSP-085
title: Generalize the resolver scoring tier helpers
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-ref
milestone: m-11
dependencies: []
ordinal: 85000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Landed 2026-07-05 (`581f3c0`). `MuxTier` +
  `PrProvenanceTier` collapsed into shared
  `ProvenanceTier` enum with `from_provenance` +
  `label` methods. Both scoring fns now use the shared
  type; discriminant values byte-identical to pre-H-REF-003
  so resolver behavior is preserved.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-REF-003`
