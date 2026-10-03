---
id: CSP-508
title: >-
  `worktrunk` backend (create/remove) behind `PATH` autodetection + `[worktree]
  backend` config
status: Done
assignee: []
created_date: '2026-07-28 12:43'
labels:
  - h-wt
milestone: m-18
dependencies: []
ordinal: 546000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Landed: trait create/remove (default Unsupported) + `WorktrunkBackend` over a `WtRunner` seam (003a); `[worktree] backend = auto|git|worktrunk` + `resolve_mutation_backend` (003b). Argv verified end-to-end against real `wt`.

- Scope (settled 2026-08): worktrunk `wt` v0.43 CLI (binary `wt`).
  Argv (category-4 subprocess, ADR 0087 / ADR 0092):
  - create: `wt -C <repo> switch --create --no-cd [--base <ref>]
    <branch>` (`--no-cd` = headless automation; no `-x` so it
    creates without launching anything).
  - remove: `wt -C <repo> remove --yes --foreground [--force]
    <branch>`.
- `WorktreeBackend` gains `create` / `remove` (default
  `Unsupported`, mirroring `MuxBackend`); `WorktrunkBackend`
  shells out via a `WtRunner` seam (fake for tests). `list`
  delegates to the git porcelain path (worktrunk worktrees are
  git worktrees). git backend keeps mutation `Unsupported`.
- `[worktree] backend = auto | git | worktrunk` (default `auto`:
  worktrunk when `wt` on PATH, else read-only; `git` forces
  read-only; `worktrunk` requires `wt`). A resolver picks the
  CLI/TUI mutation backend from config + PATH.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WT-003`
