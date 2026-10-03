---
id: CSP-390
title: 'ADR: command palette surface and minibuffer scope'
status: To Do
assignee: []
created_date: '2026-06-05 18:40'
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
