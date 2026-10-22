---
id: CSP-686
title: Rebuild every gone pinned slot at once
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-679
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: low
type: feature
ordinal: 744000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
After a reboot every mux is a ghost. The agent-deck "restore my deck" flow is one gesture that rebuilds every pinned slot whose target is gone.

- Scope: a CLI command and a Pinned view action that build one plan covering every gone slot, shown before running, continuing past individual failures and reporting them.
- Tests: plan tests for several slots with mixed outcomes.
- Manual checks: reboot or kill the tmux server and restore every pinned mux at once.
- Blockers: the Pinned view ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 One gesture rebuilds every gone pinned slot after showing the combined plan
- [ ] #2 A failure in one slot doesn't stop the others and is reported
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
