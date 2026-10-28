---
id: CSP-715
title: Reproduce and fix the suspected text-entry defects
status: To Do
assignee: []
created_date: '2026-10-27 20:26'
labels:
  - tui-framework
  - tuifw-cleanup
milestone: m-25
dependencies: []
documentation:
  - docs/plans/tui-framework/inventory/03-text-entry-and-forms.md
priority: medium
type: bug
ordinal: 801000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The inventory found four text-entry problems by reading (slice 03). `~/x` passes path validation and is committed unexpanded to a CLI that does not expand it. A multi-line paste submits a form, because bracketed paste is not enabled. Alt-B and Alt-F word motion is probably inert, because `tui-input` binds META while crossterm reports ALT. Invalid input is rejected silently in the Controls max-age editor, the empty worktree branch prompt, the new mux form, and the empty pin rename. These are fixed first, with behavior-level tests, so `LineEdit`, `TextPrompt`, and `PathField` start with a regression suite that already passes.

Scope: Reproduce each with a test. Apply the smallest fix in the current code: expand `~` when the path value is committed, enable bracketed paste in the runtime and ignore or flatten embedded newlines in single-line fields, map ALT to word motion, and show a status message for each silent rejection. Record any defect that does not reproduce.

TUI changes:
- Consistency wins: `~` paths work; multi-line paste cannot submit a form; word motion works; rejected input explains itself
- Regressions or behavior changes: a pasted multi-line value is flattened or rejected instead of acting on the first line

Tests and checks: Tests exercise each field at the behavior level and are written so they move to the new controls unchanged. Manual check by pasting a multi-line value into the rename prompt in a live terminal.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Each suspected defect is reproduced and fixed, or recorded as not reproducible
- [ ] #2 A committed path value beginning with `~` is expanded before it reaches the command
- [ ] #3 Bracketed paste is enabled and a multi-line paste into any single-line field does not submit it
- [ ] #4 Alt-B and Alt-F move by word in every text field
- [ ] #5 Each silent rejection (max-age, empty branch, new mux, empty pin rename) shows a message
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
