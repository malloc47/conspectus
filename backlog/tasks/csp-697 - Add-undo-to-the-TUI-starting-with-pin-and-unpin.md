---
id: CSP-697
title: 'Add undo to the TUI, starting with pin and unpin'
status: Done
assignee: []
created_date: '2026-10-24 02:32'
updated_date: '2026-10-24 02:54'
labels:
  - tui
milestone: m-23
dependencies: []
priority: medium
type: feature
ordinal: 773000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Unpinning removes a pin along with its overrides and can make a row vanish from the Pinned view, and p now unpins in one keypress, so a slip is costly. An undo key restores the last pin or unpin exactly. The undo mechanism should extend to other reversible actions later.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 An undo key reverses the last pin or unpin in the session, restoring a removed pin with its name, overrides, saved values, and position in the pin order
- [x] #2 Repeated undo steps back through earlier pin and unpin operations, up to a bounded history, and says when there is nothing to undo
- [x] #3 After an unpin the toast tells the operator the undo key, and the focus returns to the restored row
- [x] #4 Undo is listed in help, the key tables, and the docs, and every undoable action declares its inverse so new undoable actions must be classified
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the u key and a bounded undo stack; unpin records the full pin and position, undo restores it exactly and returns to its row; every store op is classified undoable or not by an exhaustive match. Covered by store, executor, reducer, consistency, and real-tmux end-to-end tests.
<!-- SECTION:FINAL_SUMMARY:END -->
