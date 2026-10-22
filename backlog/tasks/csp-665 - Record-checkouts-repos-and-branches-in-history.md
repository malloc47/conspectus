---
id: CSP-665
title: 'Record checkouts, repos, and branches in history'
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-662
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: high
type: feature
ordinal: 723000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Worktrees and repos are rebuildable (through worktrunk and `git clone`), so they need history records. Discovery's scope depends on scan roots, so a repo missing from this run's scan must not be treated as gone. Gone needs a targeted check of the recorded path.

- Scope: records for linked and primary checkouts (repo record id, root, branch, head commit, upstream) and repos (common dir, remotes, primary checkout root, default branch); presence checks every cycle: missing path with an existing parent is gone, missing parent or a permission error is unknown, existing is live; `[history.ignore]` globs for these kinds.
- Tests: tests for a removed worktree, a deleted repo, an unmounted parent, a scan without the repo's root (stays live), and a worktree whose branch changed.
- Manual checks: remove a linked worktree with `wt remove` and confirm its record goes gone with its branch and head kept.
- Blockers: the history store ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Checkouts and repos discovered in any run get history records with the recipe fields from the ADR
- [ ] #2 A record is gone only after a targeted check finds its path missing under an existing parent
- [ ] #3 A run with different scan roots never changes another repo's presence
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
