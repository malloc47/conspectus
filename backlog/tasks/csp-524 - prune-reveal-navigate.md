---
id: CSP-524
title: prune + reveal/navigate
status: Done
assignee: []
created_date: '2026-08-05 02:51'
labels:
  - h-wt
milestone: m-18
dependencies: []
ordinal: 562000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
**Lock/unlock dropped**: worktrunk exposes neither, and ADR 0092 deliberately routes worktree mutation through worktrunk (not raw git), so lock/unlock have no sanctioned backend — revisit if a backend gains them or an ADR extends the envelope to git worktree-admin ops. **Prune** wires to `wt step prune` (remove worktrees already merged into the default branch) as `WorktreeBackend::prune` + `WorktreeCaps.can_prune`; CLI `worktree prune [--dry-run] [--yes]` (confirms unless dry-run/--yes) and a TUI Repo-context `Prune merged worktrees` menu action -> `StoreOp::WorktreePrune`. NOTE: this is a *merged-cleanup*, distinct from git's stale-admin `prune` that the discovered `prunable` flag reflects — labels say so. **Reveal/navigate**: the previously-filtered `RevealCheckout` / `RevealSessions` actions now resolve a jump target at menu-open (containing checkout row from an agent/mux; first live session row from a worktree) and commit `Msg::SelectRow`; offered only when a target resolves. Full suite green (2022).
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-WT-008`
