---
id: CSP-856
title: 'TUI: count each session once in a group header (N)'
status: To Do
assignee: []
created_date: '2026-11-12 22:42'
updated_date: '2026-11-12 22:43'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 941000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
A group header's `(N)` counts the session rows beneath it, so a session listed twice beneath one header counts twice: in the PRs view, a checkout with two pull requests on one branch lists the branch's sessions under each PR. The waiting count beside it (`! N`, ADR 0210) counts each session once, so such a header can read `(2)  ! 1` over one waiting session. The two counts should agree on what one session is.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 A session listed under two pull requests of one group counts once in that header's `(N)`, as in its waiting count
- [ ] #2 Headers whose sessions are each listed once draw as before (the reference frames are unchanged)
- [ ] #3 docs/tui-columns.md says `(N)` counts sessions, each once
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
