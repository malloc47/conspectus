---
id: CSP-828
title: 'TUI: tell a finished turn you have seen from one you have not'
status: To Do
assignee: []
created_date: '2026-11-12 01:48'
labels:
  - live-state
  - ls-tui
  - needs-decision
milestone: m-26
dependencies:
  - CSP-823
documentation:
  - docs/plans/live-state/tui-options.md
priority: low
type: enhancement
ordinal: 914000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Six surveyed tools split "ready" into new and seen, which drives the attention count down as the operator works. Where "seen" lives is open: viewer memory, derived read-only from the mux's last attach time, or a persisted observation (a write under ADR 0143, and a flicker source in agent-deck).

Scope: Per the operator's decision, with `since` already kept accurate by the model.

TUI changes: Ready rows gain a seen distinction.

Tests and checks: Seen/unseen transitions on attach and on view; no flicker on refresh.

Blockers: Operator decision 6 and the cell story.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Pending the operator's decisions
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
