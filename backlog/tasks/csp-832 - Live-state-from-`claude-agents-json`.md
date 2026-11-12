---
id: CSP-832
title: Live state from `claude agents --json`
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
updated_date: '2026-11-12 02:25'
labels:
  - live-state
  - ls-policy
  - needs-decision
milestone: m-26
dependencies:
  - CSP-812
documentation:
  - docs/adr/0207-live-state-sources-beyond-hooks-need-their-own-decision.md
  - docs/plans/live-state/research/01-claude-code.md
priority: low
type: enhancement
ordinal: 918000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Claude Code documents `claude agents --json` as the supported way to read session state from outside, with busy, waiting, and idle statuses and what a waiting session is waiting for. It closes the Esc-interrupt and approval-granted gaps but is a new subprocess with an unknown cost, and it may start a background supervisor.

Scope: First check that running it does not start a supervisor and measure its cost. Then an ADR, a `docs/capabilities.toml` entry (ADR 0117), an adapter that runs it only to repair stale `working` and `waiting` entries, and fixtures for the output shapes.

TUI changes: Fewer stuck `working` and `waiting` states.

Tests and checks: Fixture tests; the supervisor check recorded in the ADR.

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
