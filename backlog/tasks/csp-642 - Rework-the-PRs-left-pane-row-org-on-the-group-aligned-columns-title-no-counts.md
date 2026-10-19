---
id: CSP-642
title: >-
  Rework the PRs left-pane row: org on the group, aligned columns, title, no
  counts
status: Done
assignee:
  - '@claude'
created_date: '2026-10-19 21:57'
updated_date: '2026-10-19 22:19'
labels:
  - tui
dependencies:
  - CSP-641
priority: medium
type: enhancement
ordinal: 700000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The PR rows repeat `org/repo#N` under a group that already names the repo, put the local-branch marker before the number, show a `(0)` attached count on almost every row, and give an operator no hint what the PR is about. Operator request: move the org to the group name and show only `#N` on rows, put the marker after the number, show the PR title in gray as the last content column, align the other columns like the other views, and drop the counts.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Where a group holds PRs from one forge repo, its header names the owner/repo and its PR rows show only #N; with several forge repos, or under state grouping, rows keep owner/repo#N
- [x] #2 Each PR row reads: kind glyph, number, local-branch marker, state, age, branch, title
- [x] #3 State, age, and branch columns are padded to the widest value in the tree so they align across groups; the title takes the remaining width in gray and is truncated with an ellipsis
- [x] #4 A draft open PR shows draft in the state column; the attached-session count is no longer shown on PR rows
- [ ] #5 Row-tree and renderer tests cover the label rule, alignment, truncation, and a missing title; PR frame snapshots are reviewed and an ADR records the layout
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
PR rows now read kind glyph, `#N`, local-branch marker, state, age, then the local branch (dim) and the title (gray) in the remaining width, cut with an ellipsis; number, state, and age align across the whole tree. A group of one forge repo leads with its `owner/repo` and its rows show only `#N`; mixed-repo groups and state grouping keep `owner/repo#N`. An open draft reads `draft`, and the attached-session count is gone from PR rows. Rows are laid out against the width left after the scrollbar gutter. Verified with fmt, clippy `-D warnings`, the full nextest suite (2516 passed; new row-builder tests for the label rule and new renderer tests for alignment, truncation, missing title/age, draft, and branch cap), reviewed PR frame snapshots, `tui --snapshot` against the live 90-PR world at 150 and 70 columns, and a regenerated README `prs-view.png` (ADR 0131).
<!-- SECTION:FINAL_SUMMARY:END -->
