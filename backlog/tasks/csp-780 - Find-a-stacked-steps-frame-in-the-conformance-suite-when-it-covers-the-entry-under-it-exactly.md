---
id: CSP-780
title: >-
  Find a stacked step's frame in the conformance suite when it covers the entry
  under it exactly
status: To Do
assignee: []
created_date: '2026-10-29 11:04'
labels:
  - tui-framework
milestone: m-25
dependencies: []
priority: low
ordinal: 866000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The conformance suite (`tuikit::testkit::conformance`) finds a modal's frame by the border corners that differ between the screen with the modal and the screen after `Rig::close_top`. A step opened over a menu with `ModalStack::push_step` (ADR 0172) can sit at exactly the menu's rect, as a worktree dialog does when it has as many rows as the menu or both fill a 40x10 screen; then no corner differs and the checks report "draws no frame". CSP-740 worked around it in the host: its step cases use a worktree with three live sessions so the remove confirmation is taller than the menu, and the 40x10 body check compares with the whole flow closed. A suite that knew the rect each entry drew (the overlay page's "Rendered areas" seam) would check any step at any size.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The conformance checks find the frame of a step whose rect equals the rect of the entry under it
- [ ] #2 The host step cases no longer depend on worktree facts chosen to size the step apart from the menu
- [ ] #3 The overlay or testkit control page describes how a step's frame is found
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
