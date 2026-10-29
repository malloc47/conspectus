---
id: CSP-777
title: Compute the launch form's default command off the draw path
status: To Do
assignee: []
created_date: '2026-10-29 05:50'
labels:
  - tui-framework
milestone: m-25
dependencies:
  - CSP-737
priority: low
ordinal: 863000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The mux launch form's derived `command` row calls `LaunchDefaults::default_argv` from its renderer (`default_hint` and `default_argv` in `src/tui/widgets/mux_launch.rs`, both reached from `command_line`). When `atelier` is on `PATH`, that check runs `discovery::atelier::in_atelier_workspace` on the cwd: a `canonicalize` and a walk up the tree looking for the workspace config file, twice per frame drawn. CSP-735 made the path fields themselves look at the disk only when their text changes; this is the one remaining per-frame file system read in the forms. The pin edit form's `default_argv` has the same shape.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The launch form decides its default command when the harness or the cwd changes and its renderer reads the kept answer
- [ ] #2 A test shows drawing the form twice performs no atelier workspace lookup
- [ ] #3 The pin edit form's default argv follows the same rule, or its own reason for not doing so is recorded
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
