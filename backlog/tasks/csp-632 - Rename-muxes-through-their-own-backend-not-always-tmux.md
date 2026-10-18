---
id: CSP-632
title: 'Rename muxes through their own backend, not always tmux'
status: Done
assignee:
  - '@claude'
created_date: '2026-10-18 01:27'
updated_date: '2026-10-18 04:51'
labels:
  - h-ext
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: medium
type: bug
ordinal: 690000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every mux rename goes through the tmux runner, whatever backend the mux belongs to. `rename::MuxNativeRename` carries only the mux id (`src/rename.rs:36`), so its executors cannot pick a backend: the CLI (`src/cli/rename.rs:120,183,275`, always `SystemTmux`) and the TUI executor's alias-lockstep and mux renames (`src/tui/runtime/executor.rs:175,264`). Renaming a zellij session, or a session linked to one, runs `tmux rename-session` against a tmux session that may share the name. Error messages also say "tmux" for every backend. Found while fixing CSP-618.

- Scope: carry the mux's backend in the rename plan (from its `MuxSessionNode`) and run the rename through that backend's runner (`discovery::system_mux_backend`). A backend without rename support (zellij today) is reported as such, naming the backend; for alias renames the alias is still written. Error text names the backend.
- Tests: plan tests show the backend carried from the node; a zellij rename on the CLI fails naming zellij without a tmux call; the TUI alias rename of a zellij-linked session writes the alias and reports the unsupported mux rename; tmux renames behave as before.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Mux rename plans carry the mux backend
- [x] #2 CLI and TUI mux renames run through the mux's own backend and never call tmux for a non-tmux mux
- [x] #3 A backend that cannot rename is reported by name, and an alias rename still writes its alias
- [x] #4 Session-rename lockstep targets the bare native session name, which tmux accepts (it rejected the prefixed id with "can't find session")
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. `rename::MuxNativeRename` gains `backend`, `target` (the bare native session name), and `socket`, all read from the `MuxSessionNode`. `plan_session_rename` looks the candidate mux up in the snapshot (no node, no lockstep rename); `plan_mux_rename` already has the node.
2. One helper, `discovery::mux_runner_for(backend, preferred, slot)`, returns the injected runner when it drives that backend and the system runner otherwise. The CLI rename, both TUI executor renames, and the TUI adopt rename use it.
3. Executors rename `target` on `socket`, never the prefixed id, and their messages name the backend. An alias rename still writes its alias when the mux rename is unsupported.
4. Tests: plan tests for the new fields; CLI and TUI renames of a zellij mux never call the tmux runner; a lockstep rename passes the bare name to the runner; existing rename tests keep passing.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
- `rename::MuxNativeRename` now carries `backend`, `target` (the bare native session name), and `socket`, read from the `MuxSessionNode`; `plan_session_rename` skips the lockstep rename when the linked mux node is gone.
- Folded in, same root cause (recorded as AC 4): the session-rename lockstep (CLI `rename session`, TUI `R` on a session) and CLI `rename mux` passed the prefixed id `tmux:<name>` as the tmux target. tmux reads that as session `tmux`, window `<name>`, and refuses ("can't find session: tmux"), so those renames had always failed against real tmux; unit tests used fixtures whose ids lacked the prefix. Only the TUI mux-row rename stripped it.
- `discovery::mux_runner_for` picks the caller's runner for its own backend and the system runner otherwise; the CLI rename, the TUI's new `rename_mux_via_backend` (used by both TUI rename executors), and the TUI adopt rename go through it. Messages name the backend. The operations guide notes the behavior.
- Validation: end to end on a private tmux server, `conspectus rename mux tmux:editor renamed` failed on the previous build ("tmux session `tmux:editor` not found on this server") and renamed `editor` to `renamed` on this one. New tests: production-shaped plan tests (tmux and zellij, missing node), `mux_runner_for`, TUI and CLI rename routing (bare target reaches tmux; zellij never reaches tmux). `just check` green (2427 tests), release build clean.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Mux renames now go through the mux's own backend, and they work against real tmux again. The rename plan carries the backend plus the bare session name from the mux node; a shared `mux_runner_for` picks the runner for the CLI rename, both TUI rename paths, and adopt. Fixing the plan also fixed a long-standing bug: the session-rename lockstep and CLI `rename mux` passed the prefixed `tmux:<name>` id, which tmux always rejected. Verified end to end on a private tmux server (old build fails, new build renames), new routing and plan tests, and `just check` (2427 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
