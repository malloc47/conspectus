---
id: CSP-237
title: 'CLI: `conspectus rename` command tree'
status: Done
assignee: []
created_date: '2026-05-23 02:59'
labels:
  - h-rename
milestone: m-11
dependencies:
  - CSP-236
  - CSP-239
  - CSP-234
ordinal: 286000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `conspectus rename session <id> [<name>] [--no-mux]
  [--clear]` and `conspectus rename mux <id> [<name>] [--clear]`.
  `<name>` and `--clear` are mutually exclusive; missing both is an
  error. Imperative pattern from Phase 5 — no `--dry-run`, no `--yes`.
  Use the short row-id resolution from `CSP-130`. Mux rename never
  writes an alias row (per ADR 0029 stability rule); only the native
  tmux name changes. Session rename invokes the lockstep helper from
  `CSP-239` for the default-lockstep behavior.
- Tests: CLI smoke tests for each command shape, error handling for
  mutually-exclusive flags, fake-runner-backed assertion that lockstep
  invokes both alias write and tmux rename.
- Blockers: `CSP-236`, `CSP-239`, `CSP-234`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-RENAME-007`
