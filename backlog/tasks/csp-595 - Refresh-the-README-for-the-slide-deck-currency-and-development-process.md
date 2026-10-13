---
id: CSP-595
title: 'Refresh the README for the slide deck: currency and development process'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-13 01:23'
updated_date: '2026-10-13 01:29'
labels:
  - rel
milestone: m-20
dependencies: []
priority: high
ordinal: 644000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The README is the source material for a slide deck about Conspectus. Since CSP-540.04 it has drifted: the numbers table is dated 2026-09-30, the ADR count says 97, forge discovery describes pre-ADR 0115/0116 behavior, the TUI tour omits the view digits and the refresh keys, and `just check` gained rustdoc.

Scope: true up every claim against the binary and repo, and expand "How Conspectus is built". Add the roles of the operator and the agents, a worked example of one change end to end, the project's arc by month, and the drift tests and generated assets that keep docs honest.

Tests: README link check; `git diff --check`; spot-check commands and flags against `--help`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every command, flag, recipe, and count in the README matches the current repo
- [x] #2 The README explains the development process: roles, a change end to end, the project arc, and how drift is caught
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
CHANGELOG.md's unreleased entry brought in line with the README in 173f9ba.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed in 2a8e46d. Checked every command and flag against `--help`, the recipes against the justfile, and the node and relation counts against the model (10 and 22). Refreshed the numbers table to 2026-10-12: 983 commits, 116 ADRs, 636 stories, 2,309 tests. Documented forge discovery per ADRs 0115-0116 and the view and refresh keys. Expanded the process section with roles, the CSP-594 change end to end, the project arc, drift tests, and generated screenshots. README link and anchor check and `git diff --check` pass. Re-run the numbers right before the talk.
<!-- SECTION:FINAL_SUMMARY:END -->
