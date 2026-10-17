---
id: CSP-618
title: Stop zellij pins from launching and attaching through tmux
status: To Do
assignee: []
created_date: '2026-10-17 03:09'
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
- [ ] #1 Launching a zellij pin from the CLI never creates or attaches a tmux session
- [ ] #2 Launching a zellij pin from the TUI never probes or attaches tmux
- [ ] #3 A backend that cannot launch fails up front with an error naming that backend
- [ ] #4 Tests cover a non-tmux pin on both the CLI and TUI launch paths
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
