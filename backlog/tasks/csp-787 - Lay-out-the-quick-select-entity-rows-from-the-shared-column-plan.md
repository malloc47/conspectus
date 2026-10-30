---
id: CSP-787
title: Lay out the quick-select entity rows from the shared column plan
status: To Do
assignee: []
created_date: '2026-10-30 05:25'
labels:
  - tui-framework
  - tuifw-data
milestone: m-25
dependencies:
  - CSP-753
priority: low
type: enhancement
ordinal: 873000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
CSP-753 unified the hit columns of the search modal's content rows and the Search view's hit, pane, and pane-hit rows into one declaration. The quick-select entity rows (All scope) still use their own column split, because they need two width rules the shared plan does not express yet. Add those rules to the column schema generically and move the entity rows onto the shared declaration, so a search result has one column definition everywhere.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The quick-select entity rows are laid out from the shared column plan, and the two missing width rules are added to tuikit::columns with tests
- [ ] #2 Quick-select entity rows look the same at wide widths and drop cells by the one narrow-width policy, checked with conspectus tui --snapshot at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
