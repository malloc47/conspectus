---
id: CSP-684
title: Fall back to the mux name for session display names
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-667
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: medium
type: feature
ordinal: 742000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Today a pin's `display_name` aliases its bound session everywhere. Slot ids now show only in the Pinned view, and many Claude Code sessions have empty titles, so sessions would lose a readable name. The operator expects slot names and mux names to largely match.

- Scope: display-name precedence `alias > title > mux name > id-suffix` in the TUI and tables, where the mux name comes from the live mux or the ghost that last hosted the session.
- Tests: precedence tests in row builders and table output.
- Manual checks: check untitled sessions in a named mux read as the mux name.
- Blockers: the graph projection ticket.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 An untitled session shows its live or last-hosting mux's name, and aliases and titles still take precedence
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
