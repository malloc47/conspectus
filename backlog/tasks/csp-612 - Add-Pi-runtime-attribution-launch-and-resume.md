---
id: CSP-612
title: 'Add Pi runtime attribution, launch, and resume'
status: To Do
assignee: []
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 03:05'
labels:
  - h-pi
milestone: m-22
dependencies:
  - CSP-610
references:
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/cli.md
  - >-
    https://github.com/earendil-works/pi/blob/main/packages/coding-agent/docs/environment-variables.md
documentation:
  - docs/adr/0058-pin-session-continuity.md
  - docs/adr/0117-capability-manifest-and-traced-backstop.md
  - docs/capabilities.toml
priority: medium
type: feature
ordinal: 661000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Discovering Pi sessions is only half the value: the TUI also needs to tie a live `pi` process in a tmux or zellij pane to its session, launch fresh Pi sessions from pins, and resume a selected session. Those surfaces all read through the `HarnessAdapter` registry (`runtime_signature`, `launch_argv`, `resume_argv`, theme colors), so this story fills in the Pi adapter's entries and the capability manifest.

Pi specifics: the process title is `pi` (`pi-rpc` in RPC mode), and `pi` is short enough that the loose `command_substrings` match in `cross_link` would misfire on `pip`, `api`, `pipx`, and the like. Pi does not keep its session file open, so fd attribution is weak; the hook extension story covers precise attribution. Resume is `pi --session <id>`, which searches the current project first, so the launch cwd must be the session's cwd. Launching `pi` is a new program for `exec::command` and needs a `docs/capabilities.toml` entry (ADR 0117).

Manual checks: run `pi` in a tmux pane and confirm the TUI attributes the pane to the session; resume a Pi session from the TUI; launch a fresh Pi session from a pin; check row label and color with `conspectus tui --snapshot`.

Blockers: the discovery adapter story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The Pi `RuntimeSignature` matches the exact basenames `pi` and `pi-rpc` and classifies `pi-rpc` as the Pi ADR decided; tests show `pip`, `pipx`, `api-server`, and `spin` panes are not attributed to Pi
- [ ] #2 Session-key extraction finds Pi UUIDv7 ids in commands such as `pi --session <id>` and `pi --fork <id>`
- [ ] #3 `launch_argv` returns `pi`, and `resume_argv` returns `pi --session <id>` run from the session cwd
- [ ] #4 `docs/capabilities.toml` lists `pi` with its ADR, and `tests/capabilities_static.rs` and `tests/capabilities.rs` pass
- [ ] #5 The TUI theme has a `harness_pi` identity color that can be overridden like the other harness colors, and the filter menu lists Pi
- [ ] #6 `conspectus tui --snapshot` output shows a Pi row with its badge and color
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
