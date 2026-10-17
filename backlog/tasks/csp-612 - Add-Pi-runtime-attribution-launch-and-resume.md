---
id: CSP-612
title: 'Add Pi runtime attribution, launch, and resume'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-17 03:02'
updated_date: '2026-10-17 13:52'
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
- [x] #1 The Pi `RuntimeSignature` matches the exact basenames `pi` and `pi-rpc` and classifies `pi-rpc` as the Pi ADR decided; tests show `pip`, `pipx`, `api-server`, and `spin` panes are not attributed to Pi
- [x] #2 Session-key extraction finds Pi UUIDv7 ids in commands such as `pi --session <id>` and `pi --fork <id>`
- [x] #3 `launch_argv` returns `pi`, and `resume_argv` returns `pi --session <id>` run from the session cwd
- [x] #4 `docs/capabilities.toml` lists `pi` with its ADR, and `tests/capabilities_static.rs` and `tests/capabilities.rs` pass
- [x] #5 The TUI theme has a `harness_pi` identity color that can be overridden like the other harness colors, and the filter menu lists Pi
- [x] #6 `conspectus tui --snapshot` output shows a Pi row with its badge and color
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `RuntimeSignature::command_token_basenames` (exact match on each token's basename in pane-command scans) and teach `cross_link::command_harnesses` to use it. Every other adapter sets it empty.
2. Pi signature: basenames `pi` and `pi-rpc` for process trees and pane tokens, no substrings, no fd patterns, generic UUID key extraction, and `pi-rpc` classified as Background.
3. `resume_argv` is `pi --session <id>`; `tui/resume.rs` already runs it from the session cwd.
4. Theme: a default `pi` color (bright blue), the flat `harness_pi` key, and `[tui.theme.harness].pi` through the registry. Document it in operations.md.
5. Add `pi` to `programs.harness` in `docs/capabilities.toml` with ADR 0121.
6. Tests: pane-token and process-tree matching including the look-alikes, an end-to-end link, `pi-rpc` Background (mutation-checked), key extraction, argv, theme config, and filter options. Snapshot check for the badge color.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Pi now takes part in runtime attribution, launch, and resume.
- `RuntimeSignature` gains `command_token_basenames`, an exact basename match per command token in pane-command scans (ADR 0121). Pi uses it in place of substrings.
- The Pi signature matches `pi` and `pi-rpc` exactly, classifies `pi-rpc` as a Background process, has no fd patterns, and extracts UUIDv7 keys.
- `resume_argv` is `pi --session <id>`, run from the session cwd by the existing resume path.
- The theme has a bright-blue `pi` badge, overridable with `harness_pi` or `[tui.theme.harness].pi`.
- `docs/capabilities.toml` lists `pi` under `programs.harness` with ADR 0121.
- The operations and adapter-guide docs are updated.

Verification:
- `just check`: 2358 tests, including `capabilities_static` and the traced `capabilities` test.
- New cross_link tests:
  - Pane tokens match `pi`, `atelier exec pi`, `/…/bin/pi`, and `pi-rpc`, but not `pip install`, `pipx`, `api-server`, `spin up`, `python3 -m pip`, `vim api.py`, or `pinentry`.
  - Process trees match by exact basename.
  - A `pi --session <id>` pane links its session, and look-alike panes link nothing.
  - A `pi-rpc` child doesn't register as a human agent or claim the pane. I confirmed this test fails with the Background check removed.
- pi_tests cover key extraction (`--session`, `--fork`, a session-file path, and the custom-id limitation), `pi_is_background_process`, and the launch/resume argv.
- Config tests cover both theme override forms, and a controls test covers the harness filter list.
- Manual: `tui --snapshot` against the fixtures shows Pi rows with a bold, reversed bright-blue badge (ESC[94m).

Pi is not installed on this machine, so the live tmux attribution, resume, and pin-launch checks were not done.
<!-- SECTION:FINAL_SUMMARY:END -->
