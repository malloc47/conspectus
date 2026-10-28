---
id: CSP-775
title: Give the left pane tree the list cursor marker
status: To Do
assignee: []
created_date: '2026-10-28 19:16'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-751
priority: medium
ordinal: 861000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
House rule 1 of ADR 0151 puts the `marks::POINTER` marker in a 2-cell gutter before the cursor row of every list, so the cursor stays visible without color. The selection extraction gave every menu, form, picker, search list, checklist, Messages, and the right pane that gutter, but kept the left pane's tree as it was: its rows start with the disclosure and kind-glyph columns that `docs/tui-columns.md` documents, and a gutter moves every column and every documented frame by two cells. ADR 0159 decision 6 leaves the gutter to the tree's column schema, where the tree's columns are decided and its frames are regenerated anyway. Until then the tree is the one list whose cursor row is shown by its highlight alone (reversed and bold focused, bold unfocused).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The tree draws its rows through `tuikit::selection::row` (or the tree control built on it), so the cursor row starts with the pointer and every other row with two blank cells
- [ ] #2 No column moves when the cursor moves, and every view keeps its documented column grid one gutter in
- [ ] #3 The frames in `docs/tui-columns.md` are regenerated with `just tui-columns` and its prose says where the marker sits
- [ ] #4 The consistency suite checks the tree like the other lists: one marked row, the marker alone finding it with the selection pair emptied
- [ ] #5 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, checked with `conspectus tui --snapshot` at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
