---
id: CSP-048
title: Resolver scoring for branch ↔ pull request
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4
milestone: m-5
dependencies:
  - CSP-047
ordinal: 46000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a `BranchHasForgePr`-specific comparator in
  `src/resolve/mod.rs` so that, when a branch has multiple plausible
  PRs, the preferred candidate is the most-recently-updated open
  non-draft PR, with closed/merged/draft state demoted to tie-breakers.
  Every losing candidate stays in `candidate_links` and is recorded as
  a competing link plus a `Conflict` diagnostic. Ignored and
  overridden candidates continue to be skipped.
- Tests: table-driven resolver tests for zero, one, and multiple PRs
  per branch; open vs closed vs merged ranking; draft demotion;
  ignored / overridden state handling.
- Manual checks: inspect resolved relationships for a branch with two
  open PRs and confirm losers remain visible.
- Blockers: `CSP-047`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `compare_branch_pr` in `src/resolve/mod.rs` with a
`PrScore` (provenance tier > state rank > non-draft > recency >
confidence > link id). State ranks open > merged > closed > other.
Reads `state` / `is_draft` / `updated_epoch` from the link's
`source_metadata.fields` (populated by the github fragment
builder). Nine new resolver tests cover open-vs-merged,
open-vs-closed, draft demotion, recency tie-break, declared
override, conflict diagnostic, ignored/overridden skip, and the
zero-candidate case.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-004`
