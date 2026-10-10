---
id: CSP-586
title: 'Survive a deleted launch directory in the CLI, TUI, and server'
status: Done
assignee: []
created_date: '2026-10-10 01:24'
updated_date: '2026-10-10 01:46'
labels: []
milestone: m-11
dependencies: []
priority: high
type: bug
ordinal: 628000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Reported 2026-10-09: the operator launched the TUI from a directory that was later removed, and pin launches from that TUI then failed. Every discovering command (`pin launch`, `pin list`, `graph`, `table`, `node show`, `alias list`, `declared list`, `worktree list`) fails from a deleted CWD with a bare `Error: No such file or directory (os error 2)`, because each calls `std::env::current_dir()?`. The TUI launches pins by running `conspectus pin launch` as a subprocess, which inherits the dead CWD. `serve` captures its startup CWD as the default scan root, and `normalize_scan_root` aborts the whole discovery run on a missing root, so a daemon whose launch directory is removed fails every tick and serves a stale snapshot.

Operator decision (2026-10-09): the CWD stays an optional extra scan root and the anchor for the project config walk (ADR 0012), but nothing may fail because it is missing. When it is gone, discovery runs without it and config loads from the user file alone. Authoring commands that default their target to "here" (`pin create`, `mux new`, `worktree new`, `declared add`, `alias set`) keep that default and report a clear error naming the flag to pass. The TUI resume action spawns the harness resume command in the TUI's inherited CWD rather than the session's own, which is the same class of bug.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Survey commands (graph, table, node show, pin list, pin launch, alias list, declared list, worktree list, refresh, serve) succeed from a deleted CWD, discovering from configured roots, observed paths, and the pin-store registry
- [x] #2 An implicit CWD scan root that disappears after launch is dropped on the next TUI refresh or serve tick instead of failing discovery; explicit --scan-root paths keep strict validation
- [x] #3 Config loading falls back to user config when the CWD is unavailable
- [x] #4 Commands that default a target path to the CWD fail with an actionable message naming the flag to pass, not a bare os error
- [x] #5 Launching a pin from a TUI whose launch directory was deleted succeeds
- [x] #6 TUI resume spawns the harness in the session's cwd, not the TUI's
- [x] #7 A test guards against new unchecked std::env::current_dir() calls outside the shared helper
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add a `process_cwd()` helper that returns `Option<PathBuf>`, plus a `cwd_for_default_target(flag)` helper for authoring commands that turns a missing CWD into an actionable error.
2. Survey commands: scan roots are flags, then config, then the CWD only when it is available. The config loader takes an optional anchor and skips the project walk when there is none.
3. TUI and serve: carry the implicit CWD root separately from explicit roots and re-check it exists before each discovery run.
4. Resume: carry the session cwd on `ResumeTarget::Launch` and spawn with `current_dir`.
5. Add a hygiene test that fails on direct `std::env::current_dir()` calls outside the helper. Record the convention in an ADR and update design.md and operations.md.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Plan held, with two refinements. `ScanRoots::resolve` takes the launch directory as a parameter so callers read the CWD once. `resolve_resume_target` doesn't check that the session cwd exists, because the status-bar hint calls it on every render; a deleted session cwd fails the spawn and is reported as a failed launch.

Validation:
- `tests/cli_deleted_cwd.rs` runs graph, table, pin list, alias list, declared list, worktree list, and refresh from a deleted CWD. It shows a project pin through `--scan-root`, launches a project pin into a private tmux server with the exact argv the TUI uses (`pin launch <id> --no-attach --scan-root <pin cwd>`), and checks that `mux new`, `worktree new`, and `hook init --scope project` name `--cwd`, `--repo`, and `--scope user`.
- `src/cwd_tests.rs` covers the launch-dir root dropping out once deleted and returning when recreated, plus flag > config > launch-dir precedence. `src/config_tests.rs` covers `load(None)` keeping user config and skipping the project walk. `src/tui/resume_tests.rs` and `app_tests` cover resume carrying the session cwd and `NoCwd` without one.
- Manual: an isolated `conspectus serve` started in a temp dir; after `rmdir` of that dir, `conspectus refresh` returned "refreshed via daemon" and `status` showed every class ok. Against the live graph, pin list, graph, table, node show, and a TUI snapshot all succeed from a deleted dir, and `mux new` reports the `--cwd` hint.
- `just check` passes (2215 tests, clippy, rustdoc, diff check).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The CLI, TUI, and `serve` no longer fail when their launch directory is deleted (ADR 0111). Only `src/cwd.rs` reads the working directory now. Survey commands use it as an extra scan root and config anchor while it exists and otherwise discover from configured roots, observed paths, and the pin-store registry with user config alone. The TUI and `serve` re-check the launch-dir root before every refresh or tick, while explicit `--scan-root` stays strict. Authoring commands that default to "here" name the flag to pass, and TUI resume runs in the session's cwd. `tests/cwd_hygiene.rs` blocks new direct `current_dir()` reads. Verified by `tests/cli_deleted_cwd.rs` (including a project pin launch with the TUI's argv into a private tmux server), unit tests for `ScanRoots`, config, and resume, a manual `serve` run whose launch dir was removed mid-run, a live-graph run from a deleted dir, and `just check`.
<!-- SECTION:FINAL_SUMMARY:END -->
