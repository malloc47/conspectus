---
id: CSP-352
title: Document blocked-session and continue workflows
status: To Do
assignee: []
created_date: '2026-06-03 12:44'
labels:
  - h-continue
milestone: m-11
dependencies:
  - CSP-351
ordinal: 230000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update `docs/operations.md` and any TUI help/docs with how
  usage-limit detection works, how to inspect the parsed resume
  time, how to schedule/list/cancel a pending continuation, and the
  safety limits around stale sessions or unrecognized message
  formats.
- Tests: docs-only `git diff --check`.
- Blockers: `CSP-351`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CONTINUE-006`
