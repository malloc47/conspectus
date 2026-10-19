---
id: CSP-639
title: Set the PR local-branch filter and ranking from config
status: Done
assignee:
  - '@claude'
created_date: '2026-10-19 21:36'
updated_date: '2026-10-19 21:45'
labels:
  - tui
  - forge
dependencies: []
priority: low
type: enhancement
ordinal: 697000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
ADR 0129 added a local-branch filter and a float-tracked-PRs toggle to the PRs view but left them out of the config file, so an operator who wants untracked PRs hidden or tracked ones ranked first has to set it in the Controls overlay on each machine. The other view filters (`harness`, `max_age`, `mux_state`) already have `[tui.views.<name>.filters]` keys.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 [tui.views.prs.filters] accepts pr_branch (a list of tracked and untracked) and float_tracked_prs_top (a bool), as an inline table or an array of tables
- [x] #2 Entries OR together per dimension: tracked and untracked together mean no restriction, and any entry that floats wins
- [x] #3 An unknown pr_branch value emits a config diagnostic naming the key and the valid values, and the valid entries still apply
- [x] #4 docs/operations.md documents the keys, and tests cover single, array, both-values, and invalid cases
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`[tui.views.<view>.filters]` now accepts `pr_branch` (a list of `tracked` and `untracked`) and `float_tracked_prs_top`, as an inline table or an array of tables. Entries OR together: both values mean no restriction, and any entry that floats wins. An unknown value emits a config diagnostic naming the key and the valid values while valid entries still apply. Documented in `docs/operations.md` (ADR 0130). Verified with fmt, clippy `-D warnings`, and the full nextest suite (2503 passed), including new config tests for single, array, both-values, and invalid cases.
<!-- SECTION:FINAL_SUMMARY:END -->
