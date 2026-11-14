---
id: CSP-844
title: Honor CODEX_HOME for Codex discovery
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 10:19'
updated_date: '2026-11-14 23:51'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: medium
type: bug
ordinal: 929000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Codex keeps its state (`sessions/`, `state_<N>.sqlite`, `logs_<N>.sqlite`) and its `config.toml` under `$CODEX_HOME` when it is set, and `conspectus hook init|status|remove codex --scope user` already follow it. Discovery does not. It reads `CONSPECTUS_CODEX_STATE`, else `$HOME/.codex` (`LocalDiscoveryConfig::from_env`), so an operator who relocates Codex's home gets hooks that do report live state while discovery never shows the sessions they describe. The entries then bind to nothing, with no message saying why. This came up while CSP-815 moved the Codex installer to per-event entries. It mirrors CSP-838 for Claude Code's `CLAUDE_CONFIG_DIR`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 With CODEX_HOME set (non-empty) and no CONSPECTUS_CODEX_STATE override, discovery reads Codex rollouts (sessions/), the state database (state_<N>.sqlite), and the log database (logs_<N>.sqlite) under $CODEX_HOME, through the same rule hook init|status|remove codex use
- [x] #2 CONSPECTUS_CODEX_STATE still wins when set, and $HOME/.codex stays the default when CODEX_HOME is unset or empty
- [x] #3 A session only a hook has reported, with its rollout under a moved CODEX_HOME, gets the state scope discovery gives it
- [x] #4 Tests run in a scratch home with CODEX_HOME set (and unset, and with the override also set); docs/operations.md lists the variable and the capability manifest names $CODEX_HOME/config.toml
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Codex's rule beside Claude Code's (ADR 0239): `codex::HARNESS_DIR` = `CODEX_HOME` when set and not empty, else `~/.codex`; discovery's root is `CONSPECTUS_CODEX_STATE` first. Unit tests: set, unset, empty, override also set.
2. `LocalDiscoveryConfig::from_env` and `hook init|status|remove codex --scope user` both use it (the installer already followed CODEX_HOME; it now shares the rule, and an empty CODEX_HOME no longer means a relative config.toml). The state and log database readers take the same root, so they follow.
3. A hook-only Codex session under a moved home gets discovery's state scope: `state_scope_for_transcript` takes the parent of the rollout's `sessions` directory, not only a `.codex` ancestor.
4. Test helpers that inherit the environment drop CODEX_HOME. Scratch-home integration tests: rollouts and the state database under $CODEX_HOME are read, the override wins, ~/.codex stays the default.
5. Capability manifest: `$CODEX_HOME/config.toml` (and its temporary file) under hook instead of the comment, with a traced install/remove scenario under a moved CODEX_HOME.
6. operations.md lists CODEX_HOME and drops the note that such sessions are not discovered; CHANGELOG.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
`codex::HARNESS_DIR` (ADR 0239) feeds both `LocalDiscoveryConfig::from_env` (now via `harness_state_roots_in`, env-injected) and `codex_config_path`. The state and log database readers take the same configured root, so all three follow (unit test `harness_state_roots_follow_each_harnesss_own_directory_variable`; integration test reads a rollout and a state-database-only thread under $CODEX_HOME). `state_scope_for_transcript` now finds the home by layout (the nearer of the parent of `sessions/` and an enclosing `.codex`), so a hook-only session under a moved home gets discovery's identity. Small visible change: an empty CODEX_HOME counts as unset for `hook init codex` too (it used to mean `./config.toml`).
Verified: codex_tests (rule, state scope), discovery_tests, cli_smoke discovery_reads_codex_sessions_and_state_under_codex_home (unset, empty, set, override also set), traced capabilities with CODEX_HOME moved, just check: 5788 passed, 1 skipped.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Codex discovery now follows `CODEX_HOME` (when set and not empty) for rollouts, the state database, and the log database, through the same `HarnessDir` rule `hook init|status|remove codex` use (ADR 0239); `CONSPECTUS_CODEX_STATE` still wins and `~/.codex` stays the default. A hook-only Codex session under a moved home gets the state scope discovery gives it. Test helpers drop an inherited `CODEX_HOME`; the capability manifest names `$CODEX_HOME/config.toml` and the traced test installs under it. operations.md lists the variable and drops the "not discovered" note; CHANGELOG under Fixed. Verified by unit and scratch-home integration tests and `just check` (5788 passed, 1 skipped).
<!-- SECTION:FINAL_SUMMARY:END -->
