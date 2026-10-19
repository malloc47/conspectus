---
id: CSP-643
title: Refuse actions on a row that still awaits the post-hand-off refresh
status: Done
assignee:
  - '@claude'
created_date: '2026-10-19 23:27'
updated_date: '2026-10-19 23:28'
labels:
  - tui
dependencies: []
priority: high
type: bug
ordinal: 701000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After returning from a mux, the mux row and its linked sessions stay dimmed with a spinner until the background refresh lands (ADR 0108), but the TUI still let the operator act on them. Pressing Enter to attach before the status updated could fail, since the mux the row shows may be gone or changed. ADR 0108 chose to leave the rows live; the operator asked for those actions to be refused until the row is current.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 While the selected row awaits the hand-off refresh, attach (Enter or a), resume, and pin launch show a status hint and do nothing
- [x] #2 Rename, the worktree menus, pin remove, rebind, and adopt are refused the same way; read-only actions (move, fold, view, copy, filters, search) still work
- [x] #3 Rows the hand-off did not touch stay actionable, and the actions work again once the refresh settles, including after a failed refresh
- [x] #4 An ADR amends 0108, and tests cover the reducer guard and the runtime action classification
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
While the selected row still awaits the post-hand-off refresh (the hand-off's mux, its linked sessions, and its mux-candidate rows), attach (Enter or `a`), resume, and pin launch are refused in the reducer with a toast, and rename, the worktree menus, pin remove, rebind, and adopt are refused in the runtime with the same status-bar text. Moving, folding, view, copy, search, filters, and refresh still work, rows the hand-off did not touch stay actionable, and the block lifts when the refresh settles (a failed refresh also settles it). ADR 0132 amends 0108. Verified with fmt, clippy `-D warnings`, and the full nextest suite (2519 passed), including new tests that attach/resume/launch are refused while pending and work before and after, that untouched rows stay actionable, and which actions the runtime guards. Not exercised against a live tmux hand-off in this session.
<!-- SECTION:FINAL_SUMMARY:END -->
