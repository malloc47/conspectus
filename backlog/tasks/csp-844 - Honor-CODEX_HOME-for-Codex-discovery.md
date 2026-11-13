---
id: CSP-844
title: Honor CODEX_HOME for Codex discovery
status: To Do
assignee: []
created_date: '2026-11-12 10:19'
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
- [ ] #1 With CODEX_HOME set and no CONSPECTUS_CODEX_STATE override, discovery reads Codex sessions, the state database, and the log database under $CODEX_HOME
- [ ] #2 CONSPECTUS_CODEX_STATE still wins when set, and HOME/.codex stays the default
- [ ] #3 Tests run in a scratch home with CODEX_HOME set, and docs/operations.md lists the variable
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
