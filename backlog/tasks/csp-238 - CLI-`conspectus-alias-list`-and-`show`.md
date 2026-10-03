---
id: CSP-238
title: 'CLI: `conspectus alias list` (and `show`)'
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies:
  - CSP-237
ordinal: 287000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: read-path counterpart to the rename write commands. Operators
  will want to audit overlays that hide harness-native titles. Mirrors
  `conspectus declared list` shape (`src/cli.rs:1180+`). Add `alias show
  <id>` if list-only feels thin during review.
  `conspectus alias list [--store local|global|all]`.
- Tests: CLI snapshot tests for empty, single-store, both-stores, and
  mixed-with-declared cases.
- Blockers: `CSP-237`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-008`
