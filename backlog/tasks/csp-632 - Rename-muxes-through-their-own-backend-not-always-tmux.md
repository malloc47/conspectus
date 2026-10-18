---
id: CSP-632
title: 'Rename muxes through their own backend, not always tmux'
status: To Do
assignee: []
created_date: '2026-10-18 01:27'
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
- [ ] #1 Mux rename plans carry the mux backend
- [ ] #2 CLI and TUI mux renames run through the mux's own backend and never call tmux for a non-tmux mux
- [ ] #3 A backend that cannot rename is reported by name, and an alias rename still writes its alias
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
