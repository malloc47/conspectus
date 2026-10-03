---
id: CSP-507
title: '`WorktreeBackend` trait + registry + thin `git` read/list backend…'
status: Done
assignee: []
created_date: '2026-07-28 12:43'
labels:
  - h-wt
milestone: m-18
dependencies: []
ordinal: 545000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
`WorktreeBackend` trait + registry + thin `git` read/list backend; fold worktree records into `Checkout` discovery with linked-vs-primary + branch metadata. Landed in five by-concern commits: CheckoutNode `WorktreeMeta` (kind + lock/prune); `WorktreeBackend` trait + `SystemGitWorktree` (`git worktree list --porcelain` parser); git discovery enumerates each repo's worktrees (incl. out-of-root) as Checkout nodes; read-only `conspectus worktree list`; and TUI detail-pane field + a checkout-group marker (quiet for plain primaries). One deviation from the note below: worktree checkouts are stamped with the `git` provider key (produced by the git provider, on the git cadence) rather than a separate `worktree` key — simpler and avoids a second freshness-gate slice.

- Scope (settled 2026-08-03): the read/list foundation only.
  Today git discovery probes worktree state for the single
  checkout it's pointed at (`GitProbeResult::is_linked_worktree`)
  but never enumerates a repo's *other* worktrees; 002 closes
  that gap. Decisions:
  - Enumerate ALL worktrees of every discovered repo via
    `git worktree list --porcelain`, emitting a `Checkout` node
    per worktree even for paths outside the scan root (so
    worktrees you haven't cd'd into are visible).
  - Enrich `CheckoutNode` with linked-vs-primary + locked +
    prunable metadata (branch already modeled).
  - Runs in the **git** provider class (30s TTL, one
    `git worktree list` per repo; shares git's eviction cadence).
    New `worktree` provider key mapped to `ProviderClass::Git`.
  - Read-only `conspectus worktree list [<repo>]` CLI ships in
    002 (exercises the backend directly). Mutation CLI (`new`/
    `rm`) + TUI actions stay in 004.
  - TUI: minimal linked/primary + locked/prunable marker in the
    detail pane and checkout group label. No new view/actions.
- Build order (reviewable commits by concern):
  1. Model: `CheckoutNode` worktree metadata (rkyv-compatible).
  2. `WorktreeBackend` trait + `WorktreeRecord` + registry
     (mirror `MuxBackend`); thin `git` backend + `--porcelain`
     parser.
  3. Discovery: enumerate per repo, emit/enrich `Checkout`
     nodes, stamp `worktree` provenance.
  4. CLI: `conspectus worktree list`.
  5. TUI: linked/primary + lock/prune marker.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WT-002`
