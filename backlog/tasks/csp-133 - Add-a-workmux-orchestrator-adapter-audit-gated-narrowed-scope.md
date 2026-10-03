---
id: CSP-133
title: 'Add a workmux orchestrator adapter (audit-gated, narrowed scope)'
status: To Do
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - h-agentmux
milestone: m-11
dependencies:
  - CSP-131
ordinal: 237000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: workmux's runtime mapping (tmux window names + active
  agent state) is largely a MUXPROC subset. The audit in
  `CSP-131` should focus on workmux's two artifacts that
  are plausibly non-overlapping: (a) per-worktree
  `<worktree>/.workmux/` files — project-rooted intent / labels /
  history that survive process exit and fit Conspectus's
  persistence guardrails cleanly; (b) `~/.local/state/workmux/
  agents/` resurrect state, which can describe sessions that
  aren't currently running. If both turn out to be inert mappings
  of what MUXPROC sees live, close the item. Otherwise implement
  an adapter scoped to those two artifacts: walk `.workmux/`
  during normal scan traversal, read the resurrect-state files,
  and emit candidate links only for the surviving evidence (no
  pane ↔ harness duplication).
- Tests: fixture tests over a tree containing `.workmux/`
  directories plus a fake `~/.local/state/workmux/agents/`
  layout; resurrect-state covering an exited session.
- Manual checks: run against a real workmux install if available.
- Blockers: `CSP-131`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-AGENTMUX-006`
