---
id: CSP-509
title: >-
  `conspectus worktree new/rm` CLI + menu-first TUI actions gated on a
  mutation-capable backend
status: Done
assignee: []
created_date: '2026-07-28 12:43'
labels:
  - h-wt
milestone: m-18
dependencies: []
ordinal: 547000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope (settled 2026-08): create-only semantics — `worktree new`
  creates the worktree+branch but does NOT launch an agent
  (launching stays the pin system's job).
  - CLI: `worktree new <branch> [--base <ref>] [--repo <path>]`;
    `worktree rm <branch> [--repo <path>] [--force]`.
  - Live-session guard on `rm`: refuse (listing the live agent/mux
    sessions rooted in the worktree, via the resolved
    session↔checkout links) unless `--force`.
  - TUI (only when a mutation backend is present, menu-first):
    "New worktree…" on a Repo node (branch-name prompt), "Remove
    worktree" on a Checkout node (guarded + confirm).
- Status: **CLI + guard landed (004a)**, smoke-verified with real
  worktrunk (create → git worktree list shows it → remove).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): CLI (`H-WT-004a`) and TUI (`CSP-509.02`) both
landed.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WT-004`
