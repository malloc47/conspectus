---
id: CSP-674
title: Rebuild worktrees through worktrunk
status: Done
assignee: []
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 15:06'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-672
  - CSP-665
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0092-worktree-backend-seam.md
priority: medium
type: feature
ordinal: 732000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A worktree slot or history record is repo + branch + location. Rebuilding goes through worktrunk (ADR 0092), so Conspectus never mutates git itself. Worktrunk's path template decides where the worktree lands, which may differ from the recorded root; the operator accepted that for now, and a separate ticket investigates why paths diverge.

- Scope: a worktree executor running `wt -C <repo> switch <branch>`, or `wt switch --create --base <last head> <branch>` when the branch is gone; when the new root differs from the recorded one, a new record linked by `rebuilt_from`, slots targeting the old record retargeted as part of the operator's rebuild, and a note in the outcome; plans warn when a `location` override can't be honored; the repo as a prerequisite when it's gone.
- Tests: executor tests over a fake worktrunk runner for an existing branch, a deleted branch, and a moved path.
- Manual checks: remove a worktree, then rebuild it from History.
- Blockers: the rebuild engine and checkout recording tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A gone linked worktree rebuilds for its branch, recreating a deleted branch from its last head
- [x] #2 A rebuild that lands at a different path links the records and moves slots to the new one, telling the operator
- [x] #3 No git command other than through worktrunk runs
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Worktree rebuild goes through `wt switch` (`rebuild/worktree.rs`), recreates a deleted branch from its last head, links the records and moves slots when the path differs, and calls a location override advisory. The capability manifest allows `wt switch` and no other git mutation beyond the clone. Verified by `worktree_tests` and the traced capability test.
<!-- SECTION:FINAL_SUMMARY:END -->
