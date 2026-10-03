---
id: CSP-240
title: TUI text-input widget implementation
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies:
  - CSP-233
ordinal: 289000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: per ADR 0030. Lives in new `src/tui/widgets/input.rs`. Exports
  `TextInputState`, `TextInputWidget`, and `handle_key` returning
  `InputOutcome::{Continue, Confirm(String), Cancel}`. Centered modal
  overlay, 60-col width cap, 3-row height for the rename variant.
  Status-bar shows `Enter confirm · Esc cancel` while open. Designed
  so `CSP-193` and `CSP-175` adopt without changes.
- Tests: insta snapshot tests for empty / typed / wide-terminal /
  narrow-terminal layouts. Reducer-level tests for the
  confirm/cancel/passthrough outcomes.
- Blockers: `CSP-233`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-010`
