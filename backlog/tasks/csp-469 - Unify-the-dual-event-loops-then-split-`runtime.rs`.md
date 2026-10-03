---
id: CSP-469
title: 'Unify the dual event loops, then split `runtime.rs`'
status: Done
assignee: []
created_date: '2026-07-01 22:56'
labels:
  - h-hyg
milestone: m-11
dependencies: []
ordinal: 100000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Folded into `CSP-498` (see
`docs/tui-architecture-review.md` R5) — the loop unification lands
as an event union + subscriptions rather than a parameterized
refresh source, and the `runtime.rs` split follows it. Tracked
there.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-HYG-008`
