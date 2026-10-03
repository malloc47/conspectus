---
id: CSP-150
title: Expand the `sessions` column pool
status: Done
assignee: []
created_date: '2026-05-18 21:28'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 127000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`SESSIONS_COLUMNS` gained five opt-in columns
(`checkout`, `branch`, `repo`, `fork`, `declared`). Each
extractor walks the candidate-link graph to resolve the cell:
`checkout` matches a session's `cwd` against `CheckoutId.root`;
`branch` follows `CheckedOutBranch` from the matched checkout
and strips `refs/heads/`; `repo` returns the checkout's
`RepoId.common_dir`; `fork` finds the fork that records the
session as a `ChildSession` target and renders the fork label;
`declared` reports the strongest declared candidate's state
(`declared` / `ignored` / `overridden`) by walking the raw
`snapshot.candidate_links` (so ignored/overridden links surface
through the otherwise active-only `by_source_relation` index).
`SnapshotView` now retains a reference to the underlying
`GraphSnapshot` for that purpose. The default column set is
unchanged. The `activity` column is deferred per the CSP-098
soft-blocker note. Three new unit tests cover checkout/branch/
repo, the fork column, and the declared column's link-state
mapping; one CLI integration test exercises the seven-column
selection via `--columns`. All 374 tests pass.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-010`
