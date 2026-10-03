---
id: CSP-247
title: CLI + TUI suggest surface
status: To Do
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-ai-naming
milestone: m-11
dependencies:
  - CSP-245
  - CSP-246
  - CSP-235
  - CSP-240
ordinal: 349000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `conspectus rename session <id> --suggest [--accept N]` returns
  N candidate names; operator picks one or accepts the first.
  TUI `s` key opens a candidate-list overlay backed by the alias write
  path from `CSP-235`. Picker reuses the input-widget overlay
  pattern from `CSP-240`.
- Tests: fake-LLM-runner CLI tests; TUI reducer tests for the suggest
  overlay open/pick/cancel paths.
- Blockers: `CSP-245`, `CSP-246`, `CSP-235`,
  `CSP-240`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-AI-NAMING-003`
