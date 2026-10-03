---
id: CSP-122
title: Detect agent-deck multi-repo checkouts as a workspace provider
status: Done
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-agentmux
milestone: m-11
dependencies: []
ordinal: 232000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/discovery/agent_deck.rs` ships the
`AgentDeckDiscovery` provider, wired into `discover_local_with`
via `LocalDiscoveryConfig::agent_deck_root` (defaults to
`$HOME/.agent-deck/multi-repo-worktrees`, opt-out via
`CONSPECTUS_DISABLE_AGENT_DECK`, override via
`CONSPECTUS_AGENT_DECK_ROOT`). Emits
`WorkspaceNode { provider = "agent-deck" }` + symlink-only
`WorkspaceContainsRepo` candidate links with the same
`logical_path` / `member_path_kind` source-fields shape generic
workspace uses, so the column formatter is provider-uniform.
Unit + integration tests cover two-symlink, one-symlink,
broken-symlink, non-symlink-child, and multi-workspace fixtures.
See ADR 0060 for the full decision record.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-AGENTMUX-002`
