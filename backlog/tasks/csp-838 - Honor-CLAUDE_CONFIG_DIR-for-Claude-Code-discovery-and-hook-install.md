---
id: CSP-838
title: Honor CLAUDE_CONFIG_DIR for Claude Code discovery and hook install
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 06:05'
updated_date: '2026-11-14 23:41'
labels:
  - live-state
  - ls-emit
milestone: m-26
dependencies: []
priority: medium
type: bug
ordinal: 923000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude Code moves `~/.claude` (settings, projects, transcripts) to `$CLAUDE_CONFIG_DIR` when it is set. Conspectus ignores it: `hook init|status|remove claude-code --scope user` always edit `$HOME/.claude/settings.json`, and discovery looks for transcripts under `~/.claude` unless `CONSPECTUS_CLAUDE_CODE_STATE` is set. An operator who relocates the directory gets hooks installed where Claude Code never reads them and sessions that never appear, with no message saying why. Found by research note 01 (section 4.1) and confirmed while extending the installer to all fourteen live-state events.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 With CLAUDE_CONFIG_DIR set (non-empty), hook init, status, and remove claude-code --scope user use $CLAUDE_CONFIG_DIR/settings.json and leave $HOME/.claude alone; unset or empty, they use $HOME/.claude/settings.json
- [x] #2 Discovery finds Claude Code sessions under $CLAUDE_CONFIG_DIR/projects when the variable is set and CONSPECTUS_CLAUDE_CODE_STATE is not; the override still wins when both are set, and $HOME/.claude stays the default
- [x] #3 Discovery and hook install resolve the directory through one shared, unit-tested rule (set, unset, empty, override also set), so the two cannot disagree
- [x] #4 The capability manifest names $CLAUDE_CONFIG_DIR/settings.json under hook, the traced capability test runs hook init and remove with the variable set, and the integration tests run in a scratch home with the variable set
- [x] #5 docs/operations.md lists CLAUDE_CONFIG_DIR and no longer tells its users to work around it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. One rule for Claude Code's directory, in the adapter beside Pi's (`claude_code::config_dir_from_env`): `CLAUDE_CONFIG_DIR` when set and non-empty, else `$HOME/.claude`. Discovery's root (`claude_code::state_root_from_env`) is `CONSPECTUS_CLAUDE_CODE_STATE` when set, else that directory. Env-injected unit tests cover set, unset, empty, and the override also set.
2. Use it everywhere Conspectus finds Claude Code's directory: `LocalDiscoveryConfig::from_env`, `hook init|status|remove claude-code --scope user` (`$dir/settings.json`), and the hook sidecar's `sessions/<pid>.json` read. Hooks follow Claude Code's own rule only: Claude Code never reads the Conspectus override.
3. Every test helper that runs the binary with an inherited environment removes `CLAUDE_CONFIG_DIR`, so a developer's or CI's value never reaches a test.
4. Integration tests in a scratch home: init/status/remove with the variable set write and read `$CLAUDE_CONFIG_DIR/settings.json` and leave `~/.claude` alone; discovery finds sessions under `$CLAUDE_CONFIG_DIR/projects`, the override wins over it, and `~/.claude` stays the default.
5. Capability manifest: `$CLAUDE_CONFIG_DIR/settings.json` (and its temporary file) under `hook`, a new path variable, and a traced scenario with the variable set.
6. ADR 0239 records the rule for Claude Code and Codex (Codex discovery lands with CSP-844); operations.md lists the variable, drops the workaround note; CHANGELOG entry.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Hooks follow Claude Code's own rule (CLAUDE_CONFIG_DIR, else ~/.claude) and ignore CONSPECTUS_CLAUDE_CODE_STATE, which moves discovery only: Claude Code never reads it, so a hook written there would never run (ADR 0239 decision 3). Empty values count as unset, as Pi's rule does. The sidecar's sessions/<pid>.json read now follows the discovery root too. Out of scope, filed as CSP-884: fd evidence still matches only `/.claude/projects/`.
Verified: unit tests (dirs::tests, claude_config_dir_moves_discovery_and_the_hooks), cli_smoke hook_init_status_remove_claude_code_follow_claude_config_dir and discovery_reads_claude_code_sessions_under_claude_config_dir (scratch home: set, unset, empty, override also set), traced tests/capabilities.rs with the new scenario (confirmed it fails when the manifest entry is removed), just check: 5784 passed, 1 skipped.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`hook init|status|remove claude-code --scope user` and Claude Code discovery now resolve Claude Code's directory through one rule (`HarnessDir` in `src/discovery/harness/dirs.rs`, ADR 0239): `CLAUDE_CONFIG_DIR` when set and not empty, else `~/.claude`. Discovery reads `CONSPECTUS_CLAUDE_CODE_STATE` first; the hooks never follow that override. The capability manifest names `$CLAUDE_CONFIG_DIR/settings.json`, the traced capability test installs and removes with the variable set, and every test helper that inherits the environment drops the variable. operations.md lists it and replaces the workaround note; CHANGELOG under Fixed. Verified by unit and scratch-home integration tests and `just check` (5784 passed, 1 skipped). Follow-up: CSP-884 (fd evidence in a moved directory).
<!-- SECTION:FINAL_SUMMARY:END -->
