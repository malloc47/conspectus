---
id: CSP-175
title: Inline mux-picker for ambiguous `LinkedToMux` candidates
status: To Do
assignee: []
created_date: '2026-05-19 12:49'
updated_date: '2026-10-31 08:15'
labels:
  - p8
milestone: m-13
dependencies:
  - CSP-169
  - CSP-165
ordinal: 403000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Note (2026-09-30, `CSP-534.02`): `m` is no longer free; ADR 0096 bound
  it globally to the Mux action menu. The menu-first home for this
  picker is a context entry in the `m` menu, offered when the selected
  session has ambiguous mux candidates. The status line now advertises
  `Tab inspect candidates` instead of `m choose`.
- Scope: bind the `m` key (reserved in v1, see the
  keybindings table in
  `docs/implementation/phase-08-interactive-tui.md`) so it opens
  an inline picker listing every active mux candidate for the
  selected agent session — but only when ambiguity is real
  (more than one active `LinkedToMux` candidate). Pressing `m`
  on a row with a single resolved candidate is a no-op surfaced
  as a one-line status-bar reason. Picker selection drives the
  next attach action and does not mutate declared links; a
  `c` (already reserved) can confirm the picker's choice as a
  declared link in a later story. The picker uses the same
  j/k navigation and Enter/Esc semantics as the existing
  search overlay so the muscle memory carries over.
- Tests: state-machine tests for picker open / pick / cancel /
  no-op-on-single-candidate; Ratatui snapshots for an
  ambiguous-mux picker over an agent row; a regression test
  confirming `a` continues to attach to the resolver's
  preferred candidate when `m` is never pressed.
- Blockers: `CSP-169` (attach action) and the v1 keybinding
  surface from `CSP-165`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P8-014`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
TUI framework update (2026-10-31, CSP-766): the menu-first home is an entry in the Mux menu, whose entries are data drawn by `tuikit::action_menu::ActionMenu` (ADR 0171), offered only when the selection has more than one active candidate. A pick-one list is the host's picker (`src/tui/widgets/picker.rs`, a `tuikit::palette` instance over items, ADR 0174): its arrows and `Ctrl-N`/`Ctrl-P` move and `j`/`k` type into the query, so "the same j/k navigation as the search overlay" no longer holds; use the palette's keys. Status hints are generated from the key tables (ADR 0191).
<!-- SECTION:NOTES:END -->
