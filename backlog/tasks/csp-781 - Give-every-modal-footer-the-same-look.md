---
id: CSP-781
title: Give every modal footer the same look
status: To Do
assignee: []
created_date: '2026-10-29 13:01'
labels:
  - tui-framework
  - tuifw-conventions
milestone: m-25
dependencies:
  - CSP-742
priority: medium
type: enhancement
ordinal: 867000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Modal footers come in two looks after the control extractions. Menus, dialogs, and checklists draw the footer in the plain style under a blank row; forms and palettes draw it dimmed. The footer is generated from the same key tables everywhere, so the look should be one thing too (house rule 8 of ADR 0151). The CSP-742 implementer noticed this while moving the picker onto Palette. Decide which look wins, make it a single framework function every footer goes through, and keep the blank-row spacing consistent.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One framework function draws every modal footer, and menus, dialogs, checklists, forms, palettes, prompts, and pagers all use it
- [ ] #2 The footer's style and the blank row above it are the same in every modal, checked with conspectus tui --snapshot at a normal and a narrow width
- [ ] #3 Every visible change is listed in the final summary under **Regressions** and **Consistency wins**, and was checked with `conspectus tui --snapshot` at a normal and a narrow width
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
