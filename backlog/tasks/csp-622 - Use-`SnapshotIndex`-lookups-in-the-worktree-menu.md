---
id: CSP-622
title: Use `SnapshotIndex` lookups in the worktree menu
status: To Do
assignee: []
created_date: '2026-10-17 03:09'
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
- [ ] #1 Worktree-menu lookups by node id use `SnapshotIndex`
- [ ] #2 One helper answers which worktree checkout contains a path, for both callers
- [ ] #3 Worktree menu behavior is unchanged, covered by existing tests
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
