---
id: CSP-586
title: 'Survive a deleted launch directory in the CLI, TUI, and server'
status: In Progress
assignee: []
created_date: '2026-10-10 01:24'
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
- [ ] #1 Survey commands (graph, table, node show, pin list, pin launch, alias list, declared list, worktree list, refresh, serve) succeed from a deleted CWD, discovering from configured roots, observed paths, and the pin-store registry
- [ ] #2 An implicit CWD scan root that disappears after launch is dropped on the next TUI refresh or serve tick instead of failing discovery; explicit --scan-root paths keep strict validation
- [ ] #3 Config loading falls back to user config when the CWD is unavailable
- [ ] #4 Commands that default a target path to the CWD fail with an actionable message naming the flag to pass, not a bare os error
- [ ] #5 Launching a pin from a TUI whose launch directory was deleted succeeds
- [ ] #6 TUI resume spawns the harness in the session's cwd, not the TUI's
- [ ] #7 A test guards against new unchecked std::env::current_dir() calls outside the shared helper
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add a `process_cwd()` helper that returns `Option<PathBuf>`, plus a `cwd_for_default_target(flag)` helper for authoring commands that turns a missing CWD into an actionable error.
2. Survey commands: scan roots are flags, then config, then the CWD only when it is available. The config loader takes an optional anchor and skips the project walk when there is none.
3. TUI and serve: carry the implicit CWD root separately from explicit roots and re-check it exists before each discovery run.
4. Resume: carry the session cwd on `ResumeTarget::Launch` and spawn with `current_dir`.
5. Add a hygiene test that fails on direct `std::env::current_dir()` calls outside the helper. Record the convention in an ADR and update design.md and operations.md.
<!-- SECTION:PLAN:END -->
