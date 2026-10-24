---
id: CSP-684
title: Fall back to the mux name for session display names
status: Done
assignee: []
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 18:13'
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
- [x] #1 An untitled session shows its live or last-hosting mux's name, and aliases and titles still take precedence
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Session display names follow alias, then title, then the live or last-hosting mux name, then the id suffix, through one shared function used by the TUI rows, the Pinned view, and table sessions; ADR 0029 carries an amendment note. Covered by aliases_tests, sessions_tests, and table_tests.
<!-- SECTION:FINAL_SUMMARY:END -->
