---
id: CSP-777
title: Compute the launch form's default command off the draw path
status: Done
assignee:
  - '@claude'
created_date: '2026-10-29 05:50'
updated_date: '2026-10-29 07:51'
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
- [x] #1 The launch form decides its default command when the harness or the cwd changes and its renderer reads the kept answer
- [x] #2 A test shows drawing the form twice performs no atelier workspace lookup
- [x] #3 The pin edit form's default argv follows the same rule, or its own reason for not doing so is recorded
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Covered by CSP-737, which rebuilt the launch form's command row. `widgets::launch_argv::DefaultArgv` keeps the default argv and its source for one harness and cwd. `refresh` decides again only when either changes. The launch form (`MuxLaunchForm::follow`) and the pin edit form (`refresh_default`, after each key and on open) draw from that answer, so neither checks for an Atelier workspace while drawing. Tests: `drawing_the_form_twice_reads_the_default_it_kept` (mux_launch_tests) and `drawing_the_form_reads_the_default_argv_it_kept` (pin_edit_tests). Each opens a form over a directory with `atelier` on `PATH`, turns the directory into a workspace, and draws twice. The command row and the effective argv still show the harness default. One edit to the cwd then shows `atelier exec`. `the_default_is_decided_once_until_the_harness_or_the_directory_changes` (launch_argv_tests) covers the cache itself. Verified with the CSP-737 checks: fmt, clippy (-D warnings), the full isolated suite (4098 passed, 0 failed, 1 ignored), rustdoc, and `git diff --check`. No visible change.
<!-- SECTION:FINAL_SUMMARY:END -->
