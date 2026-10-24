---
id: CSP-658
title: Record the rebuild plans and executors decision
status: Done
assignee:
  - '@claude'
created_date: '2026-10-22 01:10'
updated_date: '2026-10-22 02:34'
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
- [x] #1 A new ADR records the plan model, each executor's steps and envelope category, prerequisite chaining, argv precedence including launch profiles, and the worktree location rule
- [x] #2 The ADR supersedes ADR 0094 and amends ADR 0093 (teardown leaves a ghost), ADR 0095, and ADR 0096 (adopting as a pin goes away)
- [x] #3 The ADR states that the worktree location rule is provisional pending the location investigation ticket
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Write ADR 0142 from the plan's Rebuild section.
2. Index it and put status lines on ADRs 0093, 0094, 0095, 0096, and 0098.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Recorded ADR 0142 (rebuild plans and executors): plans shown before running and printed by `--dry-run`, executors for mux (gone, agent running, agent exited, dead pane, zellij), session, worktree, and repo clone with their envelope categories, prerequisite chaining, argv precedence including launch profiles and the `atelier exec` layer, the provisional worktree location rule pending `CSP-688`, history writes at launch, and the TUI flow. Supersedes ADR 0094; amends 0093, 0095, 0096, and 0098. Verified with `git diff --check`.
<!-- SECTION:FINAL_SUMMARY:END -->
