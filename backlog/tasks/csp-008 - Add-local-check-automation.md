---
id: CSP-008
title: Add local check automation
status: Done
assignee: []
created_date: '2026-05-15 02:31'
labels:
  - p0
milestone: m-1
dependencies:
  - CSP-004
  - CSP-007
ordinal: 8000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add a `justfile` with targets for formatting, linting, tests,
  nextest, and whitespace diff checks.
- Tests: `just check` runs the complete baseline check suite.
- Manual checks: `just --list`.
- Blockers: `CSP-004`, `CSP-007`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `P0-005`
