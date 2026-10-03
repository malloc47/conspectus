---
id: CSP-165
title: 'Implement selection, focus, navigation, and filtering state'
status: Done
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies: []
ordinal: 392000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Deferred to follow-on stories (not v1 through-line blockers):
  view switching `1`–`5` (waits on the other row-tree builders
  from CSP-163 parts 2-5), `/` in-view search overlay,
  `r` refresh-intent dispatch (waits on CSP-167 to have
  something to refresh), `?` help overlay.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (v1 slice): `App` carries the row tree, snapshot,
expanded-set, selection by `RowId`, panel focus, and preview
scroll. Reducer handles `j/k/arrows`, `PageDown/PageUp`,
`Home/End/g/G`, `Enter` (expand/collapse), `Tab` (focus
cycle), and `J/K` (preview scroll). `Msg::SetData` retains
selection by `RowId` across refreshes and falls back to the
nearest visible row by index when the previously-selected id
disappears. Detail view-model recomputes eagerly on every
selection change. Initial expansion now opens the launch-context
tree and leaves unrelated trees collapsed; if no launch-context
row is known, it opens the first tree as a fallback.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-006`
