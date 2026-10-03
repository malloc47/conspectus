---
id: CSP-244
title: Docs and snapshot coverage
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies:
  - CSP-241
  - CSP-243
ordinal: 293000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update `docs/operations.md` with the new commands; update the
  Phase 8 TUI doc keybindings table
  (`docs/implementation/phase-08-interactive-tui.md`); add insta
  snapshot tests for renamed-row rendering in tree, table, and detail
  surfaces.
- Tests: doctest where applicable; `git diff --check`; insta review.
- Blockers: `CSP-241`, `CSP-243`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-014`
