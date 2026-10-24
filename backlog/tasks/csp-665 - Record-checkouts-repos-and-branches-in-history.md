---
id: CSP-665
title: 'Record checkouts, repos, and branches in history'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 12:45'
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
- [x] #1 Checkouts and repos discovered in any run get history records with the recipe fields from the ADR
- [x] #2 A record is gone only after a targeted check finds its path missing under an existing parent
- [x] #3 A run with different scan roots never changes another repo's presence
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Recorder observes checkouts and repos from fresh git provenance; CheckoutNode.head_commit and GitProbeResult.head.
2. Projection decides gone/unknown by a targeted path check.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Checkouts (repo key, root, linked or primary, branch, head commit, upstream) and repos (common dir, remotes, primary root) are recorded whenever git discovery ran this cycle, and presence comes from a targeted check of the recorded path: missing under an existing parent is gone, a missing parent or an error is unknown, an existing path is live. A scan with other roots never changes another repo's presence. Verified by projection tests (removed worktree, deleted repo, unmounted parent, present path not reached by the scan) and a CLI test that removes a real linked worktree and sees it gone with branch and a 40-character head kept. A repo with no commits is handled (`rev-parse HEAD` absence).
<!-- SECTION:FINAL_SUMMARY:END -->
