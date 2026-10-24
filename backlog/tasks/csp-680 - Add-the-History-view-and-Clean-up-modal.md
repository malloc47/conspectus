---
id: CSP-680
title: Add the History view and Clean up modal
status: Done
assignee: []
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 18:12'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-678
  - CSP-669
  - CSP-672
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: medium
type: feature
ordinal: 738000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator wants to rebuild anything observed without having pinned it. The History view lists gone records in the shared row layout, and its Clean up modal is the TUI way to reap.

- Scope: a History view of gone records, grouped by project (default) and sorted by last seen; `Enter` rebuilds; menu with pin, forget, show plan; mux rows expand to hosted sessions with "rebuild with this session"; a Clean up modal listing reap candidates with ages, each deselectable, then confirm.
- Tests: view-model, snapshot, and modal interaction tests.
- Manual checks: rebuild a gone worktree and a ghost mux from the view; reap from the modal.
- Blockers: the row renderer, history commands, and rebuild engine tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Every gone history record is listed and can be rebuilt, pinned, or forgotten from the view
- [x] #2 The Clean up modal reaps only the candidates left selected and never pinned or Unknown records
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the History view (key 5, --view history) and the Clean up modal. Rows can be rebuilt, pinned, or forgotten from the view or its row menu; Clean up lists reap candidates, preselected, and removes exactly the selection, never pinned or Unknown records. Covered by history_tests, clean_up_tests, row_menu_tests, and ui snapshots.
<!-- SECTION:FINAL_SUMMARY:END -->
