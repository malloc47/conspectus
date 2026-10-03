---
id: CSP-277
title: Fixture corpus and query regression suite
status: Done
assignee: []
created_date: '2026-05-25 21:41'
labels:
  - p9
milestone: m-14
dependencies:
  - CSP-272
  - CSP-273
ordinal: 469000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: a small library of representative graph fixtures
  (sparse-orphan-session, multi-checkout-repo, fork-ancestry-chain,
  workspace-with-prs, ambiguous-mux-candidates) and a fixture-driven
  test that runs a battery of canned queries against each, asserting
  stable result shapes. Lives alongside the existing snapshot
  fixtures.
- Tests: itself — this is the regression net.
- Blockers: `CSP-272`, `CSP-273`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tests/query_regression.rs` plus snapshots for
sparse orphan sessions, multi-checkout repos, fork ancestry,
workspace PRs, ambiguous mux candidates, and saved-view row counts.
The suite runs canned SQL against fixture snapshots materialized
through the query loader.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P9-007`
