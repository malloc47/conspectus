---
id: CSP-391
title: 'Action registry: enumerate the command surface'
status: To Do
assignee: []
created_date: '2026-06-05 18:40'
updated_date: '2026-10-31 08:15'
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

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): re-scoped. The scoped keymap (CSP-760, ADR 0192) is most of this registry for the main screen: every binding has a stable id (`search.quick`), its keys, the program's action (`Command`), a verb, a help section (the category) and text, a status rank, and an optional `available_when` guard, and the keymap's build refuses duplicate ids and overlapping keys. `Keymap::bindings` enumerates it, and `keys::rows(table)` gives an overlay table's rows. What is left: the palette's row source over those bindings (plus any action reachable only from a menu, if CSP-390 includes them), tests that every described binding is listed and that availability dims or hides a row (shared with CSP-776), and no byte-for-byte parity with the Controls overlay's help text (the `?` page is generated from the keymap now). `src/tui/actions.rs` exists and holds the menu builders; the source needs no module of its own.
<!-- SECTION:NOTES:END -->
