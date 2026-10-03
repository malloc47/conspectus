---
id: CSP-202
title: Update table and TUI projections for checkout grouping
status: Done
assignee: []
created_date: '2026-05-21 00:10'
labels:
  - h-checkout
milestone: m-11
dependencies:
  - CSP-201
ordinal: 149000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: replace single-parent checkout grouping assumptions with
  checkout/workspace-aware projection rules. Default to including
  workspace overlay groups while also allowing checkout-centric output;
  add include/exclude workspace controls before making workspace
  duplication visible by default.
- Tests: table snapshots and TUI row-tree tests showing the same
  session under workspace and checkout when appropriate, plus a
  workspace-excluded mode with no duplicate workspace rows.
- Slice landed: repo rows in the TUI sessions tree display a
  human-oriented repo source path instead of the git common-dir
  identity, with a `/.git` stripping fallback.
- Tests: `cargo test repo_group`.
- Blockers: `CSP-201`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
TUI graph grouping now prefers resolved
session→workspace context, while repo grouping remains the
workspace-excluded view. The sessions table now has an opt-in
`workspace` column exposing resolved workspace context alongside
existing checkout/repo/branch columns.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-CHECKOUT-006`
