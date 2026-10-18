---
id: CSP-618
title: Stop zellij pins from launching and attaching through tmux
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:09'
updated_date: '2026-10-18 00:00'
labels:
  - h-ext
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: medium
type: bug
ordinal: 676000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Pin validation accepts any backend in `KNOWN_MUX_BACKENDS`, which has included zellij since CSP-482, but both launch paths still assume tmux. `conspectus pin launch` always constructs `SystemTmux` (`src/cli/pin.rs:754-757`) and never reads `pin.mux.backend`. In the TUI, `PinLaunchTarget` keeps only `mux_name` and `mux_socket` (`src/tui/actions.rs:537-540`), so `execute_launch_pin` checks the session with `tmux has-session` (`src/tui/runtime/launch.rs:452`) and attaches with a hard-coded `tmux:` id through `run_tmux_attach_with_socket` (`launch.rs:101-107`). A zellij pin therefore starts a tmux session. The resolver looks for a zellij mux with that name, so the pin never binds, and if an unrelated tmux session already has that name the TUI attaches to it. Found by reading the code during the 2026-10-16 hygiene re-verification; not reproduced against a live zellij server.

- Scope: make both launch paths pick the mux backend from the pin through the `MuxBackend` seam (CSP-480), or refuse non-tmux pins at launch with an explicit unsupported-backend error until zellij launch exists. Either way, no pin launches or attaches through a backend other than its own.
- Tests: CLI and TUI launch tests with a zellij pin and a fake backend that assert no tmux call is made; existing tmux pin-launch tests pass unchanged.
- Manual checks: with a zellij pin in a scratch store, `conspectus pin launch <id>` either opens a zellij session or exits non-zero naming the unsupported backend, and `tmux ls` shows no new session.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Launching a zellij pin from the CLI never creates or attaches a tmux session
- [x] #2 Launching a zellij pin from the TUI never probes or attaches tmux
- [x] #3 A backend that cannot launch fails up front with an error naming that backend
- [x] #4 Tests cover a non-tmux pin on both the CLI and TUI launch paths
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `MuxBackend` gains `can_create_sessions()` (default `false`; `SystemTmux`, `FakeTmux`, and the `Box` delegate say `true`) so launch can refuse before any side effect (worktree creation, sidecar cleanup) instead of learning from an `Unsupported` outcome afterwards.
2. `discovery::system_mux_backend(key)` maps a registered backend key to its system runner. It ignores the discovery opt-outs, since `CONSPECTUS_DISABLE_TMUX` disables discovery, not launch.
3. CLI `pin launch`/`attach`: find the pin first, then pick the runner for `pin.mux.backend`; unknown backend fails naming it. `launch_new` checks `can_create_sessions()` first and fails naming the backend. Attach and send-keys error messages name the runner's backend instead of saying tmux.
4. TUI: `PinLaunchTarget` carries the pin's backend. A new `run_mux_attach` dispatches on the attach target's backend (tmux keeps its `switch-client` nesting path; other backends go through `MuxBackend::attach_session`). Pin launch and the general `ExecSpec::AttachMux` both use it; the `tmux has-session` probe only runs for tmux targets.
5. Scope note: the general TUI attach (`ExecSpec::AttachMux`) also always ran tmux, even for zellij muxes. It shares the helper from step 4, so it is fixed here too.
6. Tests: CLI test with a zellij pin on a private tmux server (launch fails naming zellij; no tmux session created); unit tests for `can_create_sessions`, `system_mux_backend`, the TUI launch target backend, and attach dispatch.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `MuxBackend::can_create_sessions()` (default false; tmux and its fake say true) lets launch refuse before realizing a worktree or touching the resume sidecar. `discovery::system_mux_backend(key)` maps a backend key to its system runner and ignores the discovery opt-out env vars.
- CLI `pin launch`/`attach` choose the runner from `pin.mux.backend`; attach and send-keys errors name the backend.
- TUI: `PinLaunchTarget` carries `mux_backend` (every pin-bearing row now resolves it from the snapshot pin). `run_mux_attach` routes by backend (`attach_route`); the `tmux has-session` probe runs only for tmux; `attach_return_entry` checks pane status through the target's own backend.
- Scope addition, recorded in the plan: the general TUI attach (`ExecSpec::AttachMux`) always ran tmux, even for zellij muxes. It now uses `run_mux_attach` too.
- Found, not fixed (outside this task): TUI rename flows (`execute_commit_alias_rename`, `execute_commit_mux_rename`) send mux renames through the tmux runner whatever the mux backend; `RowKind::Pin`/`PinRow` is never constructed outside tests.
- Validation: `tests/cli_pin.rs::zellij_pin_launch_fails_naming_zellij_without_touching_tmux` (launch and attach) fails on the old code, which printed "spawned `zp` (detached); attach with: tmux attach-session -t zp", and passes now; unit tests for `system_mux_backend`, `can_create_sessions`, `attach_route`, `attach_outcome`, `attach_return_entry`, and the launch target backend; `just check` green (2398 tests).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Zellij pins no longer launch or attach through tmux. The CLI picks the runner from `pin.mux.backend`, and a backend that cannot create sessions (zellij) fails before any side effect with an error naming it. The TUI carries the pin backend into its launch target and attaches through `run_mux_attach`, which routes by backend; the general TUI mux attach uses the same path, so zellij muxes no longer attach through tmux either. Verified with a CLI test on a private tmux server (reproduces the old tmux spawn, passes now), new unit tests, and `just check` (2398 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
