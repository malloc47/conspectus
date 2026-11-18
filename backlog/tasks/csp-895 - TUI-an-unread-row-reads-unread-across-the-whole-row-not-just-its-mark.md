---
id: CSP-895
title: 'TUI: an unread row reads unread across the whole row, not just its mark'
status: In Progress
assignee:
  - '@claude'
created_date: '2026-11-17 23:50'
updated_date: '2026-11-17 23:51'
labels:
  - tui
dependencies: []
ordinal: 971000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The unread behaviour (ADR 0245) bolds only the one-cell live mark, which many terminal fonts barely show, so an operator who rebuilt against main did not notice it working. The row itself should look unread: the cells that name and date the agent as well as the mark.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A session row whose live stop is unread draws its key, age, label, and preview bold as well as its mark; a read row draws none of them bold beyond what it did before
- [ ] #2 A mux row whose shown state is unread draws its name, age, and preview bold as well as its mark
- [ ] #3 An aliased session (label already bold) still reads differently when unread than when read
- [ ] #4 The cursor row, group headers, Pinned, History, Search, and PR rows are unchanged
- [ ] #5 docs/tui-columns.md, the help legend, and the ADR record the rule; tests cover read and unread rows of both kinds
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
