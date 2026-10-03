---
id: CSP-201
title: Resolve multi-context session membership
status: Done
assignee: []
created_date: '2026-05-21 00:10'
labels:
  - h-checkout
milestone: m-11
dependencies:
  - CSP-199
  - CSP-200
ordinal: 148000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: extend cross-link resolution so a session can associate with
  both a workspace and the underlying checkout/repo/branch. Preserve
  candidate evidence for each context and expose enough resolved data
  for projections to choose deduped or multi-home display.
- Tests: resolver tests for workspace-member sessions, checkout-only
  sessions, ambiguous workspace providers, and sessions with multiple
  mux candidates.
- Slice landed: cross-link inference now emits
  `AgentSession`→checkout `associated_with` candidates when the
  session cwd is at or under a discovered checkout root, choosing the
  deepest checkout for nested repo cases. Resolver multi-home semantics
  are still open.
- Tests: `cargo test checkout`.
- Blockers: `CSP-199`, `CSP-200`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Cross-link inference now also emits
`AgentSession`→workspace `associated_with` candidates from
workspace-member `logical_path`/`canonical_checkout_root` metadata.
The resolver treats `associated_with` and `workspace_contains_repo`
as multi-target relations, so distinct contexts resolve
independently while duplicate evidence for the same target still
competes normally.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-CHECKOUT-005`
