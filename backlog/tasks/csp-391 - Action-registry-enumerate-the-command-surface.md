---
id: CSP-391
title: 'Action registry: enumerate the command surface'
status: To Do
assignee: []
created_date: '2026-06-05 18:40'
labels:
  - h-cmd
milestone: m-17
dependencies:
  - CSP-390
ordinal: 516000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: walk every `Action` variant, TUI keybinding, runtime helper
  (`src/tui/runtime.rs`), and modal surface (viewer, rename, graph
  export, pinned-row launcher) and produce a structured action
  registry — one entry per user-visible command — with a stable
  command id, a short description, the category (navigation /
  session / mux / display / export / transcript / rename), default
  keybinding, and applicability guard (e.g. `pinned-row` actions
  only apply when a pinned row is selected). Keep the registry in a
  new `src/tui/actions.rs` or similar, separate from the existing
  key-dispatch map, so the command palette can enumerate it without
  importing the runtime. This is the foundation the ADR describes;
  the palette overlay (`CSP-392`) and the minibuffer
  (`CSP-393+`) both consume it.
- Tests: unit tests for the registry shape, uniqueness of command
  ids, applicability-guard coverage for at least four categories,
  and a snapshot of the full command list for discovery parity with
  the existing Controls overlay.
- Manual checks: confirm the registry matches the Controls overlay
  help text byte-for-byte (same descriptions, same keybindings);
  spot-check `conspectus tui --help` for consistency.
- Blockers: `CSP-390`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CMD-002`
