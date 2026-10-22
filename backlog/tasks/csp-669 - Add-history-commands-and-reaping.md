---
id: CSP-669
title: Add history commands and reaping
status: Done
assignee: []
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 15:05'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-667
  - CSP-668
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: medium
type: feature
ordinal: 727000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The operator needs to see what history holds, rebuild from it, and clean up old ghosts. Reaping is operator-initiated only and protects anything a pin slot targets.

- Scope: `history list [--kind] [--gone]`, `history show <ref>`, `history reap [--kind] [--older-than] [--dry-run]`, `history forget <ref>`; `[history.reap_after]` defaults (mux 14 days, checkout 30 days, PR 30 days after leaving the listing, repos only by `forget`); pinned and Unknown records never reaped; reaping removes history records only; `worktree close` forgets the gone records rooted in the worktree instead of dropping pins.
- Tests: CLI tests for each command, thresholds, pinned and Unknown protection, and dry run; a worktree close-down test.
- Manual checks: reap with `--dry-run`, then for real, and confirm tmux and the filesystem are untouched.
- Blockers: the graph projection and slot store tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 History can be listed, inspected, reaped by age and kind, and forgotten record by record from the CLI
- [x] #2 Reaping never removes a pinned or Unknown record and never touches tmux, git, or the filesystem
- [x] #3 `worktree close` forgets the worktree's gone history records
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `conspectus history list|show|reap|forget` (`src/cli/history.rs`) and `history::reap` (candidates by per-kind threshold or `--older-than`, never pinned or Unknown records, repos only with an explicit age). `worktree close` forgets the gone records rooted in the worktree except those a slot targets. Covered by `reap_tests`, `close_down_tests`, and `tests/cli_pin.rs`; the capability trace runs `history reap --dry-run` as a read command and shows reaping touches only the history store.
<!-- SECTION:FINAL_SUMMARY:END -->
