---
id: CSP-175
title: Inline mux-picker for ambiguous `LinkedToMux` candidates
status: To Do
assignee: []
created_date: '2026-05-19 12:49'
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
