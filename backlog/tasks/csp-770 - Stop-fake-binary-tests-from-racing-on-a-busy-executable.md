---
id: CSP-770
title: Stop fake-binary tests from racing on a busy executable
status: To Do
assignee: []
created_date: '2026-10-28 05:04'
labels:
  - testing
dependencies: []
priority: low
type: bug
ordinal: 856000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Tests that write a fake executable (a fake gh in the forge tests, a fake tmux in rebuild::script_tests) and then run it intermittently fail with 'Text file busy' (ETXTBSY). Three separate agents hit it while running the full suite during the TUI framework milestone: once in a forge test, and twice in rebuild::script_tests::the_tmux_commands_a_plan_shows_are_the_arguments_the_runner_executes. The test passes alone and on rerun. The usual cause is a parallel test forking while another test still holds the freshly written file open for writing, so the exec sees it busy. It is unrelated to the TUI work but costs a rerun on every full-suite run.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 The intermittent 'Text file busy' failure no longer reproduces over 20 consecutive full-suite runs, or the cause is identified and recorded with the reason a fix is not possible
- [ ] #2 Fake executables are written, synced, and closed before any test can exec them, through one shared helper
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
