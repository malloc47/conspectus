---
id: CSP-677
title: Treat exited zellij sessions as ghosts
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-672
  - CSP-618
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: low
type: feature
ordinal: 735000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Zellij keeps its own ghosts: `list-sessions` reports `EXITED - attach to resurrect`, and discovery already parses that flag. They should be ghosts in history, rebuilt by resurrecting them with zellij's layout.

- Scope: exited zellij sessions are gone in presence; the mux executor runs `zellij attach <name>` for them, falling back to creation when zellij has dropped the session.
- Tests: executor tests over a fake zellij runner.
- Manual checks: exit a zellij session and resurrect it from the Mux view.
- Blockers: the rebuild engine ticket and CSP-618.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Exited zellij sessions show as gone and rebuild by resurrection
- [ ] #2 A zellij session zellij no longer lists is recreated
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
