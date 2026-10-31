---
id: CSP-390
title: 'ADR: command palette surface and minibuffer scope'
status: To Do
assignee: []
created_date: '2026-06-05 18:40'
updated_date: '2026-10-31 08:15'
labels:
  - h-cmd
milestone: m-17
dependencies: []
ordinal: 515000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: settle the two-phase delivery (command palette first as the
  lower-risk, higher-discoverability surface; minibuffer second as
  the general-purpose prompt once the action registry is mature).
  Decide the fuzzy-match algorithm (substring with smart-case vs
  `nucleo` / `skim`-style scoring), the action registry shape (enum
  vs trait + `describe()`), whether commands are statically
  registered or discovered at runtime, how user-defined keybindings
  and aliases plug into the registry, and the overlay placement
  (CSP-240's centered modal pattern vs a bottom-anchored
  palette vs a full-screen overlay for the command-search variant).
  Record the relationship with ADR 0030 (text-input widget), ADR
  0033 (extensible keybinding config), and the existing `Controls`
  overlay. Does not introduce new crate dependencies unless the ADR
  explicitly justifies one.
- Tests: docs-only; `git diff --check`.
- Manual checks: review the ADR against the existing TUI surface,
  the Controls overlay help text, and the column-registry pattern
  from H-TBL.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CMD-001`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): re-scoped, since several questions are now settled. The match rule is the palette's (`tuikit::palette`, filtering with `every_word` over rows the host ranks, ADR 0174); the registry is the scoped keymap's bindings, each with a stable id, a verb, a help section and text, and an optional `available_when` (ADR 0192, `src/tui/keybindings.rs`), declared statically and checked at build for duplicate ids and overlapping keys; user keybindings would plug in through those stable ids (`crates/tuikit/docs/controls/keys.md`, "User remapping"); placement is a `ModalFrame` named size (ADR 0158). The text-input relationship is ADR 0030 as narrowed by ADR 0165 and 0166; the "ADR 0033" in the scope is the superseded detail-pane record, and the key-binding records are 0123, 0160, and 0192. The Controls overlay is a `SettingsPanel` (ADR 0176). Left to decide: the opening key (the keymap's build refuses an overlap), which scopes the palette lists (global and the focused pane; overlay tables?), whether menu-only actions (row menu entries) join, how a pick routes (`Command::action` today, `Msg` after CSP-802), and whether the minibuffer is a one-row `TextPrompt` host (the prompt page's seam).
<!-- SECTION:NOTES:END -->
