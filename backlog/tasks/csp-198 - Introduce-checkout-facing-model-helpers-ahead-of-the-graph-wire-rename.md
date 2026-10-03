---
id: CSP-198
title: Introduce checkout-facing model helpers ahead of the graph wire rename
status: Done
assignee: []
created_date: '2026-05-21 00:10'
labels:
  - h-checkout
milestone: m-11
dependencies:
  - CSP-197
ordinal: 145000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `Checkout` model/helpers as the canonical code-level
  vocabulary while keeping the current `Worktree` graph representation
  until the hard wire/model rename lands.
- Tests: graph JSON snapshot/round-trip tests proving checkout-facing
  helpers produce the same node identities.
- Blockers: `CSP-197`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Initial checkout-facing helpers were introduced as a staging
step, then replaced by canonical checkout graph/model names in
`CSP-204`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-CHECKOUT-002`
