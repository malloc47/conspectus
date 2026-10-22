---
id: CSP-658
title: Record the rebuild plans and executors decision
status: To Do
assignee: []
created_date: '2026-10-22 01:10'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-656
  - CSP-657
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0094-worktree-backed-pins-realized-at-launch.md
  - docs/adr/0098-pin-resume-preserves-launch-argv.md
  - docs/adr/0103-launches-keep-failed-panes.md
priority: high
type: docs
ordinal: 716000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rebuilding is what turns history and pin slots back into live entities. Today it exists only as `pin launch` for muxes and as worktree-backed pins (ADR 0094). The plan generalizes it into plans made of steps, with an executor per kind and prerequisites between them. The operator chose this milestone's scope: mux plus session resume, worktree, mux on a gone worktree, and repo clone.

- Scope: the plan model (ordered steps shown before running, `--dry-run`); executors for mux (including the agent-exited path), session, worktree, repo and primary-checkout clone; prerequisite chaining; argv precedence (slot override > recorded argv > project launch profile > user launch profile > discovered `atelier exec` layer > `HarnessAdapter::launch_argv`); the worktree location rule (accept worktrunk's path, link with `rebuilt_from`, retarget slots, warn when a `location` override can't be honored; a follow-up investigates why paths diverge); orchestrator-owned muxes have rebuild disabled; zellij resurrection through `zellij attach`.
- Tests: docs-only; `git diff --check`.
- Manual checks: none.
- Blockers: the history and pin slots ADRs.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A new ADR records the plan model, each executor's steps and envelope category, prerequisite chaining, argv precedence including launch profiles, and the worktree location rule
- [ ] #2 The ADR supersedes ADR 0094 and amends ADR 0093 (teardown leaves a ghost), ADR 0095, and ADR 0096 (adopting as a pin goes away)
- [ ] #3 The ADR states that the worktree location rule is provisional pending the location investigation ticket
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
