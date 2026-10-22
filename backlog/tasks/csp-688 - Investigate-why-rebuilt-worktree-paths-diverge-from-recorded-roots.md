---
id: CSP-688
title: Investigate why rebuilt worktree paths diverge from recorded roots
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-674
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
  - docs/adr/0092-worktree-backend-seam.md
priority: low
type: spike
ordinal: 746000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Rebuilt worktrees land wherever worktrunk's path template puts them, which may differ from the recorded root. The operator accepted that for now (the history record links old and new and slots move), but wants the deeper reasons understood before deciding whether exact locations matter.

- Scope: find out where existing worktrees come from (worktrunk, `git worktree add`, agent-deck, Atelier, other tools), how often their roots differ from worktrunk's template, and what breaks when a worktree moves (history identity, slots, sessions' recorded cwd, tools with hard-coded paths); evaluate worktrunk support for explicit paths (upstream feature or config) versus an envelope change for `git worktree add`; record findings, and an ADR if behavior should change.
- Tests: none; findings document.
- Manual checks: survey the operator's repos for worktrees and their origins.
- Blockers: the worktree executor ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Findings explain where recorded worktree roots come from and when they diverge from worktrunk's template
- [ ] #2 A recommendation states whether exact locations should be honored and how, with an ADR or follow-up tickets if so
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
