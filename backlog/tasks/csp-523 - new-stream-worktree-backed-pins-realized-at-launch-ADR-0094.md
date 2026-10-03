---
id: CSP-523
title: 'new-stream: worktree-backed pins realized at launch (ADR 0094)'
status: Done
assignee: []
created_date: '2026-08-05 02:51'
labels:
  - h-wt
milestone: m-18
dependencies: []
ordinal: 561000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A pin gains an optional `[worktree] branch` block; `cwd` is the repo anchor and the branch's worktree is resolved-or-created (from the repo default) and entered **at launch** — not at pin-write time — so the declaration stays pure and nothing is orphaned. Landed: schema (`PinEntry.worktree` + serde round-trip); launch realization in `conspectus pin launch` (`realize_worktree_cwd`: reuse existing worktree, else create via the mutation backend, re-resolve path), which the TUI inherits via its `pin launch` re-exec; the create-form worktree toggle + branch field (branch defaults to the derived id, editable; edit/rebind preserve the block from disk); `N` opens the form with the toggle pre-enabled (plain create stays on the `p` menu); and CLI parity via `pin new --worktree <branch>`. Full suite green (2015).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WT-007`
