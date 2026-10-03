---
id: CSP-394
title: 'Migrate rename, search, and view-switch prompts into the minibuffer'
status: To Do
assignee: []
created_date: '2026-06-05 18:40'
labels:
  - h-cmd
milestone: m-17
dependencies:
  - CSP-393
  - CSP-193
ordinal: 519000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: once the minibuffer exists, route the existing inline
  prompts through it: `R` (rename) opens the minibuffer pre-
  populated; `/` (search, `CSP-193`) opens the minibuffer; `v`
  (view-switch) opens the minibuffer with completion over the
  registered views. Keep the existing modal fallback for terminals
  where the minibuffer layout is impractical. Remove the standalone
  text-input modals only after the minibuffer versions have been
  exercised for at least one release cycle.
- Tests: integration tests for each migrated prompt; regression
  tests proving the removed modals no longer register keybindings.
- Manual checks: run through the full rename → search → view-switch
  flow using only the minibuffer; confirm history and completion
  work across each prompt type.
- Blockers: `CSP-393`, `CSP-193`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CMD-005`
