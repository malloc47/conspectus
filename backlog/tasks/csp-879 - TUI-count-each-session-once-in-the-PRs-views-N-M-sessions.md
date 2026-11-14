---
id: CSP-879
title: 'TUI: count each session once in the PRs view''s N/M sessions'
status: To Do
assignee: []
created_date: '2026-11-14 01:04'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 956000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Under a narrowing filter the header's `N/M sessions` in the PRs view counts the session rows the view lists (`visible_agent_session_count` in `src/tui/ui/header.rs`), so a session listed under two pull requests of one checkout counts twice and N can overstate what is listed, or even exceed M. Group headers count each session once since CSP-856 (ADR 0210, amended); the header line should agree on what one session is. Found while fixing CSP-856.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Under a narrowing filter, a session listed under two pull requests counts once in the PRs view's N
- [ ] #2 N is unchanged where each session is listed once (the reference frames are unchanged)
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
