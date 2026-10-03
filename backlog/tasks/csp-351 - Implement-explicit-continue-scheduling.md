---
id: CSP-351
title: Implement explicit continue scheduling
status: To Do
assignee: []
created_date: '2026-06-03 12:44'
labels:
  - h-continue
milestone: m-11
dependencies:
  - CSP-347
  - CSP-349
  - CSP-350
ordinal: 229000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a command and matching TUI action that schedule a
  `Continue` message for the selected blocked session at its parsed
  resume time, with flags to override the time and message text.
  The implementation must verify that the target session is still
  the same session before sending, record enough state to list/cancel
  pending jobs, and avoid project-tree cache/state writes.
- Tests: fake scheduler and fake harness sender coverage for create,
  list, cancel, missed time, target-session mismatch, custom message,
  and dry-run behavior.
- Manual checks: schedule against a disposable session using a
  near-future time, confirm the prompt is sent once, and confirm
  cancelling prevents delivery.
- Blockers: `CSP-347`, `CSP-349`, `CSP-350`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CONTINUE-005`
