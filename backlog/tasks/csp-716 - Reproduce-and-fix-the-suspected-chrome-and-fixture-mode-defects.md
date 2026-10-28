---
id: CSP-716
title: Reproduce and fix the suspected chrome and fixture-mode defects
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/07-chrome-and-explorer.md
  - docs/plans/tui-framework/inventory/06-interaction-conventions.md
priority: medium
type: bug
ordinal: 802000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Three chrome problems were found by reading (slices 06 and 07). The tab strip probably clips at 100 to 107 columns with no elision. The header probably reads `0/N sessions` outside the Sessions and Mux views. In `tui --fixture` interactive mode, a menu opened with `.` may not be dismissable, because `StaticMode::dispatch` answers every menu and form key forwarder with a status hint. The framework's `TabBar`, `StatusBar`, and `testkit` should be built from known behavior.

Scope: Reproduce each. Fix the ones that reproduce in the current code, or decide that fixture-mode interaction is dead and remove it, recording the decision. Record those that do not reproduce.

TUI changes:
- Consistency wins: the tab strip fits or elides at every width; the header count matches the active view; fixture mode is either usable or removed
- Regressions or behavior changes: none expected

Tests and checks: Snapshots at widths 90, 100, 104, 107, and 120. A test for the header count in each view.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each suspected defect is reproduced and fixed, or recorded as not reproducible
- [ ] #2 The tab strip is checked at widths 90, 100, 104, 107, and 120 and never clips mid-label
- [ ] #3 The header count reflects the active view
- [ ] #4 Fixture-mode interaction can dismiss every overlay, or its interactive mode is removed with the decision recorded
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
