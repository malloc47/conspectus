---
id: CSP-700
title: Delete the aider harness adapter and aider-only code
status: To Do
assignee: []
created_date: '2026-10-24 03:19'
updated_date: '2026-10-31 08:16'
labels:
  - h-aider-removal
milestone: m-24
dependencies:
  - CSP-699
priority: medium
type: task
ordinal: 776000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The aider adapter (`src/discovery/harness/aider.rs`) is unused. It is also the only per-repo harness, so code across discovery, the TUI, and tests treats it as a special case. This task deletes the adapter and every reference to it, and removes or keeps the aider-only seams as the ADR from CSP-699 decides.

When filed, aider was referenced from: `src/discovery/harness/{aider,mod,fixtures,fixtures_tests,harness_tests}.rs`, `src/discovery/{providers,providers_tests,cache,cache_tests,mod}.rs`, `src/dev_scenarios.rs`, `src/output/{render,table_tests}.rs`, `src/tui/{theme,theme_tests,keymap,resume,resume_tests,viewer_bridge,viewer_bridge_tests,viewer_tests,app_tests,runtime_tests}.rs`, `src/tui/runtime/executor.rs`, `src/tui/preview/value.rs`, `src/tui/rows/sessions_tests.rs`, `src/tui/widgets/badge.rs`, `examples/pantry.rs`, and `tests/tui_tmux_e2e.rs`. Re-check with `git grep -i aider` when you start; the list may have changed.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `git grep -i aider -- src tests examples` returns nothing, except for config-compatibility handling the ADR keeps on purpose
- [ ] #2 The `aider` provider no longer exists: it is gone from the provider descriptor registry, the cache provider lists, and the harness adapter registry, and `known_harness_keys` (or its equivalent) lists only the remaining harnesses
- [ ] #3 Each aider-only seam (viewer external-launch escape hatch, `resume_argv` `cwd` argument, per-repo marker scan) is removed or kept as the ADR decides; if one is kept, its doc comments no longer cite aider as the motivating case
- [ ] #4 A config that sets `harness_aider` behaves as the ADR decides, and a test covers that behavior
- [ ] #5 Loading a `graph.bin` or history store written by a build that still had aider behaves as the ADR decides, without panicking
- [ ] #6 Dev scenarios, fixtures, `examples/pantry.rs`, and the tmux e2e test no longer create or expect aider state, and the showcase scenario still renders
- [ ] #7 `docs/capabilities.toml` drops any aider-only entry, and the capability tests pass
- [ ] #8 `docs/tui-columns.md` drops the aider harness color, and its drift tests pass
- [ ] #9 `cargo nextest run --all-targets --all-features`, `cargo fmt -- --check`, and `cargo clippy --all-targets --all-features -- -D warnings` pass
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the TUI's aider references moved with the framework extraction. `src/tui/widgets/badge.rs` no longer names aider (its tests, `badge_tests.rs`, do). The harness color table is `HARNESS_COLORS` and the `harness_aider` alias a row of `ALIASES`, both in `src/tui/theme.rs`, registered with the framework theme as the role `harness.aider` (ADR 0153; `theme_tests.rs` checks one entry per registered adapter). `crates/tuikit/tests/independence.rs` lists `aider` in its domain word list; that file is outside AC #1's pathspec and the word should stay (see the CSP-699 note). `examples/pantry.rs` still names aider. Re-run `git grep -i aider -- src tests examples docs/tui-columns.md` when starting; nothing under `crates/tuikit/src` or its docs can name it.
<!-- SECTION:NOTES:END -->
