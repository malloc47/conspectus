---
id: CSP-895
title: 'TUI: an unread row reads unread across the whole row, not just its mark'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-17 23:50'
updated_date: '2026-11-18 00:40'
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
- [x] #1 A session row whose live stop is unread draws its key, label, and preview bold as well as its mark; a read row draws none of them bold beyond what it did before
- [x] #2 A mux row whose shown state is unread draws its name and preview bold as well as its mark
- [x] #3 An aliased session (label already bold) still reads differently when unread than when read
- [x] #4 The cursor row, group headers, Pinned, History, Search, and PR rows are unchanged
- [x] #5 docs/tui-columns.md, the help legend, and the ADR record the rule; tests cover read and unread rows of both kinds
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Scope narrowed from the task text on purpose: an unread row's age and harness chip keep their style (they are framework cells; the key, label, name and preview carry the weight). Recorded in ADR 0247.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
An unread session row draws its key, label (a title; an alias is bold anyway), and preview bold as well as its mark; an unread mux row draws its name and preview bold. Colors and widths are unchanged, and the age and chip keep their style (ADR 0247). Verified: consistency tests for read and unread rows of both kinds and for an aliased session; the Sessions and Mux snapshots at the showcase anchor time showed the bold runs and, after `C`, none; the full pre-main gate (just check, cargo test, separation rehearsal, just plugins, nextest with no conspectus on PATH) passed. docs/tui-columns.md and the help legend say it.
<!-- SECTION:FINAL_SUMMARY:END -->
