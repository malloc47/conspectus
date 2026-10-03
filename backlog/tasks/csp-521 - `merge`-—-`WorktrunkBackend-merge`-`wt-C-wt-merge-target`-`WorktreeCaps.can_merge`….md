---
id: CSP-521
title: >-
  `merge` — `WorktrunkBackend::merge` (`wt -C <wt> merge [target]`) +
  `WorktreeCaps.can_merge`…
status: Done
assignee: []
created_date: '2026-08-05 02:51'
labels:
  - h-wt
milestone: m-18
dependencies: []
ordinal: 559000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`merge` — `WorktrunkBackend::merge` (`wt -C <wt> merge [target]`) + `WorktreeCaps.can_merge`; TUI `MergeWorktree` action ("Merge back & close") with `ConfirmMerge` mode → `StoreOp::WorktreeMerge` / `Msg::CommitWorktreeMerge` / `execute_worktree_merge`; CLI `worktree merge <branch> [--target] [--force]`, guarded like `rm`. Merge sits in the Worktree + Mux menu contexts. Full suite green (1991).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WT-005`
