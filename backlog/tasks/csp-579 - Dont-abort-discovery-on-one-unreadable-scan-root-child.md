---
id: CSP-579
title: Don't abort discovery on one unreadable scan-root child
status: Done
assignee: []
created_date: '2026-10-02 16:58'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 583000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: `conspectus graph --refresh` run from `/tmp` fails with
  "discovery provider `generic_workspace` failed: failed to run git
  rev-parse --is-inside-work-tree: Permission denied". The generic
  workspace provider probes every child of the scan root, and a single
  child git can't enter turns into an error that ends the whole
  discovery run.
- Plan: treat a permission error on one child probe as "not a repo"
  plus a diagnostic, the way missing paths are already handled, and
  add a test with an unreadable child directory.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`GitProbe::probe` and `probe_cached` now skip directories
the process can't enter, as they already skipped non-directories
(`is_probeable_dir` checks search permission by resolving `root/.`).
Every caller benefits: generic workspace children, Atelier and
agent-deck members, and observed session cwds. No diagnostic is
emitted, since an unreadable sibling is ordinary in shared
directories like `/tmp` and would only add noise. `graph --refresh`
from `/tmp` now completes; regression tests cover the probe and
workspace discovery with a 0o000 child.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-021`
