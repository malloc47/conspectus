---
id: CSP-622
title: Use `SnapshotIndex` lookups in the worktree menu
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:09'
updated_date: '2026-10-18 00:17'
labels:
  - h-hyg
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: low
type: chore
ordinal: 680000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The worktree menu (`src/tui/widgets/worktree_menu.rs`) was written after `SnapshotIndex` (CSP-467) but looks nodes up by id with linear `snapshot.nodes.iter().find_map` scans at lines 85, 102, and 127, the case `SnapshotIndex::node` covers. It also searches for "the worktree checkout containing this path" twice with the same predicate: `checkout_row_containing` (line 157) and `checkout_containing` (line 212) differ only in what they return. The file holds 7 of the 23 production `nodes.iter()` scans left in `src/`. Audit finding C2.

- Scope: route the id lookups through `SnapshotIndex::node`, and replace the two path searches with one helper that returns the matching `CheckoutNode` for each caller to project. Path-containment scans the index does not cover can stay scans.
- Tests: existing worktree-menu tests pass; add one for the shared helper with nested worktrees.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Worktree-menu lookups by node id use `SnapshotIndex`
- [x] #2 One helper answers which worktree checkout contains a path, for both callers
- [x] #3 Worktree menu behavior is unchanged, covered by existing tests
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `context_for_node` builds one `SnapshotIndex` and resolves the selected checkout, mux, or agent node with `SnapshotIndex::node` instead of three linear `find_map` scans. (It runs once per `w` press, so a local index matches how every other consumer uses it.)
2. Replace `checkout_row_containing` and `checkout_containing` with one `worktree_checkout_containing(snapshot, path) -> Option<&CheckoutNode>`; each caller projects the row id or `(root, branch)` it needs.
3. Path-containment scans the index does not cover (`first_session_row_in`, `live_sessions_in_worktree`, `primary_worktree_root`) stay scans.
4. Tests: existing `context_for_*` tests pass unchanged; add a helper test with nested worktrees.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `context_for_node` resolves the selected checkout, mux, or agent through one `SnapshotIndex` (`SnapshotIndex::node`) instead of three linear `find_map` scans. The file's production `nodes.iter()` scans drop from 7 to 3; the rest are path-containment searches the index does not cover.
- `worktree_checkout_containing` replaces `checkout_row_containing` and `checkout_containing`; callers project the group row (`checkout_group_row`) or root and branch (`checkout_short_branch`).
- Behavior fix, found while writing the nested-worktree test the ticket asked for: both old helpers returned the first matching checkout in node order, which for a worktree nested inside another checkout's tree is the outer one. The helper now returns the innermost (deepest root).
- Validation: `containing_checkout_is_the_innermost_worktree` fails with first-match selection (left `/wt/outer`, right `/wt/outer/inner`) and passes now; the existing `context_for_*` tests pass unchanged; `just check` green (2401 tests).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The worktree menu now looks up the selected node through `SnapshotIndex` and answers "which worktree checkout contains this path" with one helper instead of two copies. That helper now returns the innermost checkout; the old copies returned the outer one when worktrees were nested. Verified with a new nested-worktree test (fails under the old first-match rule), the existing menu tests, and `just check` (2401 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
