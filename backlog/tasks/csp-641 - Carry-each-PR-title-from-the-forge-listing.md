---
id: CSP-641
title: Carry each PR title from the forge listing
status: Done
assignee:
  - '@claude'
created_date: '2026-10-19 21:57'
updated_date: '2026-10-19 22:19'
labels:
  - forge
dependencies: []
priority: medium
type: enhancement
ordinal: 699000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The PRs view row cannot show what a PR is about: the title is only fetched lazily by the preview (`gh pr view`), so it is known for a PR only after it has been selected. `gh pr list` can return `title` in the same call at no extra round trip, which makes the title available to every PR row and to `table prs`. This adds a field to the archived `ForgePrNode`, so the snapshot format version moves.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 gh pr list requests title and the parser reads it, collapsing whitespace to a single line and dropping an empty title
- [x] #2 ForgePrNode carries the title, which survives the graph JSON and the on-disk snapshot, and FORMAT_VERSION is bumped
- [x] #3 table prs has an opt-in title column
- [x] #4 Tests cover parsing (missing, empty, multi-line titles), the node field, and the JSON snapshot; an ADR records the field and the format bump
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`gh pr list` now also returns `title`; the parser reads it onto one line, `ForgePrNode` carries `title: Option<String>`, `FORMAT_VERSION` is 7, and `table prs` has an opt-in `title` column (ADR 0131). Verified with fmt, clippy `-D warnings` and the full nextest suite (2516 passed), including new parser tests for missing, empty, non-string, and multi-line titles, the node field, the `--json` field list, and the `table prs` column; and a live run that returned titles for all 90 of this machine's PRs.
<!-- SECTION:FINAL_SUMMARY:END -->
