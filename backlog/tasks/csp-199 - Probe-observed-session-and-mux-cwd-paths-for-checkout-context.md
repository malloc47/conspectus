---
id: CSP-199
title: Probe observed session and mux cwd paths for checkout context
status: Done
assignee: []
created_date: '2026-05-21 00:10'
labels:
  - h-checkout
milestone: m-11
dependencies:
  - CSP-197
ordinal: 146000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: collect distinct cwd paths from discovered agent sessions and
  mux sessions, run read-only git probes for each path, and backfill
  `Repo`, `Checkout`, and `Branch` nodes plus candidate links even
  when the cwd is outside the launch cwd or configured scan roots.
  Reserve `Ungrouped` for sessions with no usable path or context
  evidence.
- Slice landed: `discover_local_with` now probes distinct observed
  agent-session and mux-session cwd paths after initial discovery and
  merges any git repo/checkout/branch evidence before cross-link
  inference.
- Slice landed: table and TUI projections now match sessions whose cwd
  is nested under a checkout root, choosing the deepest matching
  checkout.
- Tests: `cargo test observed_session_cwd_backfills_git_context_outside_scan_roots`;
  `cargo test checkout`; `cargo test sessions_projection_optional_branch_repo_worktree_columns`;
  `cargo test prs_projection_attached_shows_agent_with_matching_cwd`.
- Blockers: `CSP-197`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Coverage now includes plain clone cwd, linked worktree cwd,
bare-repo-derived worktree cwd, nested cwd inside a checkout,
nonexistent cwd, and mux cwd outside configured scan roots.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-CHECKOUT-003`
