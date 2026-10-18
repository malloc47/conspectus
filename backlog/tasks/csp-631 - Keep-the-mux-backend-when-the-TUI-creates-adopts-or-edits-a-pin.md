---
id: CSP-631
title: 'Keep the mux backend when the TUI creates, adopts, or edits a pin'
status: To Do
assignee:
  - '@claude'
created_date: '2026-10-18 01:27'
updated_date: '2026-10-18 01:27'
labels:
  - h-ext
milestone: m-11
dependencies: []
references:
  - docs/code-hygiene-audit.md
priority: high
type: bug
ordinal: 689000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The TUI writes every pin it creates or edits with `mux.backend = "tmux"` (`src/tui/runtime/pin_store.rs:189` for create and adopt, `:295` for edit), whatever mux the pin belongs to. Two consequences:

- Saving any edit to an existing zellij pin from the TUI rewrites its stored backend to `tmux`, silently corrupting the operator's `.conspectus.toml`.
- Adopting a zellij mux row from the TUI writes a tmux pin, and renames the source mux through the tmux runner (`apply_pin_adopt_mux_rename`, `pin_store.rs:146`), which acts on whatever tmux session shares the name.

The duplicate-mux preflight (`pin_store.rs:349`) also compares only against tmux entries. Found while fixing CSP-617 and CSP-618; the CLI paths already keep the backend.

- Scope: carry the backend through the TUI create, adopt, and edit requests. Create defaults to tmux; adopt takes the selected mux row's backend; edit keeps the pin's stored backend. Route the adopt-time rename through that backend's runner, reporting an unsupported backend instead of calling tmux. Make the duplicate-mux preflight compare backend, name, and socket together.
- Tests: editing a zellij pin keeps `backend = "zellij"` in the written TOML; adopting a zellij row writes a zellij pin and never calls the tmux runner; creating a pin still writes tmux; preflight rejects a same-name same-backend duplicate and allows a same-name pin on another backend.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Editing a pin from the TUI preserves its stored `mux.backend`
- [ ] #2 Adopting a mux row from the TUI writes the row's backend and renames through that backend only
- [ ] #3 Creating a pin from the TUI still writes `backend = "tmux"`
- [ ] #4 The duplicate-mux check compares backend, name, and socket
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
