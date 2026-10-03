---
id: CSP-040
title: Wire agent and tmux discovery into local graph discovery
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-034
  - CSP-037
  - CSP-039
ordinal: 40000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: register the harness and tmux providers in local discovery so
  `conspectus graph --format json` emits repo, workspace, fork, session, and
  mux evidence from cwd/configured roots and supported local state.
- Tests: CLI tests for deterministic graph output with fake harness and tmux
  discovery, unavailable tmux, and orphan sessions.
- Manual checks: run `cargo run -- graph --format json` with a tmux smoke
  session and confirm useful output when sessions remain unlinked.
- Blockers: `CSP-034`, `CSP-037`, `CSP-039`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added a `LocalDiscoveryConfig` (harness state roots + optional
tmux runner) and a `discover_local_with` entry point. The default
`discover_local_at_roots` builds the config from the environment
(`CONSPECTUS_CODEX_STATE` / `_CLAUDE_CODE_STATE` / `_OPENCODE_STATE`
overrides, otherwise `$HOME`-relative paths, plus `CONSPECTUS_DISABLE_TMUX`
to skip the tmux provider). Discovery now also calls
`cross_link::infer` after merging fragments so session↔mux and
session↔fork candidates appear automatically. CLI integration tests run
with an isolated `$HOME` and `CONSPECTUS_DISABLE_TMUX=1`, and library
tests exercise the full chain with `FakeTmux` plus fixture state.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-009`
