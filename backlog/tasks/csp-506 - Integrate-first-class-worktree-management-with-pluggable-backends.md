---
id: CSP-506
title: Integrate first-class worktree management with pluggable backends
status: Done
assignee: []
created_date: '2026-07-28 03:30'
labels:
  - h-wt
milestone: m-18
dependencies: []
ordinal: 544000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: product design for creating / listing / removing git
  worktrees from Conspectus with a pluggable backend seam targeting
  `worktrunk` (https://github.com/max-sixty/worktrunk) as the rich
  backend and a thin built-in `git worktree` wrapper as the always-
  available fallback. Must fit the graph model (worktrees as
  checkouts and their fork/branch provenance), the ADR 0087 mutation
  envelope (worktree create/remove is git-state mutation — the design
  has to reconcile this with the "never mutates git state" guardrail,
  likely via a new sanctioned category or an explicit
  operator-initiated exception), and the existing checkout-context
  model. Deliverable is a design section in `docs/design.md` plus one
  or more ADRs (backend seam, mutation-envelope amendment) before any
  implementation stories are split out.
- Tests: design-phase; none until implementation stories land.
- Blockers: the ADR 0087 mutation-envelope reconciliation is the key
  open decision and needs operator direction.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): epic complete; `CSP-507` through
`CSP-524` landed.

Outcome (design done): operator chose "delegate mutation to external
tools." Recorded as ADR 0092 and a `## Worktree Management` section
in `docs/design.md`. Key decisions: worktrees are `Checkout` nodes
(no new node type); a `WorktreeBackend` seam splits read-only `list`
(always available, built-in thin `git` backend via `git worktree
list --porcelain`) from delegated `create`/`remove` (external
`worktrunk`, invoked as an ADR 0087 category-4 subprocess launch).
ADR 0087 prohibition 6 (never mutate git state) stays unchanged —
Conspectus never runs `git worktree add/remove` itself.
Implementation stories (follow-ups):
- **CSP-507** `WorktreeBackend` trait + registry + thin `git` read/list backend…
- **CSP-508** `worktrunk` backend (create/remove) behind `PATH` autodetection + `[worktree] backend` config
- **CSP-509** `conspectus worktree new/rm` CLI + menu-first TUI actions gated on a mutation-capable backend
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-WT-001`
