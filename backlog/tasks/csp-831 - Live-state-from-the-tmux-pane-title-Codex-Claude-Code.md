---
id: CSP-831
title: 'Live state from the tmux pane title (Codex, Claude Code)'
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
labels:
  - live-state
  - ls-policy
  - needs-decision
milestone: m-26
dependencies:
  - CSP-812
documentation:
  - docs/adr/0206-live-state-sources-beyond-hooks-need-their-own-decision.md
  - docs/plans/live-state/research/02-codex.md
priority: low
type: enhancement
ordinal: 917000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Codex writes a spinner into the terminal title while working and `[ ! ] Action Required` when it needs the operator. It is the best hook-free waiting signal and the only one that clears the instant an approval is answered. Reading it is an already-visible status surface that ADR 0028 allows only through its own decision.

Scope: An ADR (tier under ADR 0086, states the source may assert, how it yields to hook evidence), the tmux format variable read in the existing mux probe, a classifier with a golden string table, the `unknown` fallback for unrecognized titles, and the hook-health diagnostic.

TUI changes: Waiting is detected without hooks for Codex, and clears promptly after an approval.

Tests and checks: String table tests; priority rules against hook evidence.

Blockers: Operator decision 11.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Pending the operator's decision
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
