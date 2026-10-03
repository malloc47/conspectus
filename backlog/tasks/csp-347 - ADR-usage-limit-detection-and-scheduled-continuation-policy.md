---
id: CSP-347
title: 'ADR: usage-limit detection and scheduled continuation policy'
status: To Do
assignee: []
created_date: '2026-06-03 12:44'
labels:
  - h-continue
milestone: m-11
dependencies: []
ordinal: 225000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: record the provider-neutral model for a "blocked until"
  session signal, the allowed scheduler backend(s), where scheduled
  jobs live, how missed/cancelled jobs behave, and why sending a
  literal `Continue` prompt is safe enough only as an explicit action.
  Compare alternatives such as shelling out to `at`, a Conspectus
  background server, tmux `send-keys`, harness-native resume
  commands, and manual reminders.
- Tests: docs-only; `git diff --check`.
- Manual checks: review against ADR 0023, ADR 0052, and the
  read-only discovery requirements in `docs/design.md`.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CONTINUE-001`
