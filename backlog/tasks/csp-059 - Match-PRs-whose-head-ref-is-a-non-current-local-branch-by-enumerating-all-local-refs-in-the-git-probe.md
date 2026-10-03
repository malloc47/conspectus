---
id: CSP-059
title: >-
  Match PRs whose head ref is a non-current local branch by enumerating all
  local refs in the git probe
status: Done
assignee: []
created_date: '2026-05-16 03:21'
labels:
  - p4-fu
milestone: m-9
dependencies: []
ordinal: 80000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Phase 4 adapter only matches the currently-checked-out branch, so PRs for sibling branches end up as unresolved-endpoint candidate links rather than node-target links. The evidence is still preserved; the resolved relationship just goes unresolved.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`GitProbe` now enumerates local branch short refs via
read-only `git for-each-ref`, `fragment_from_probe` emits
non-current local branches as `Branch` nodes without adding
checked-out links, and the GitHub forge provider matches PR
`headRefName` values against the full local branch set. Added
provider coverage for a sibling branch PR and updated git
discovery snapshots for the newly visible branch nodes.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P4-FU-002`
