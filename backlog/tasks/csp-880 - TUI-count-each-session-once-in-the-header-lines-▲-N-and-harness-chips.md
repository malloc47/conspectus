---
id: CSP-880
title: >-
  TUI: count each session once in the header line's warning count and harness
  chips
status: To Do
assignee: []
created_date: '2026-11-14 02:03'
updated_date: '2026-11-14 02:03'
labels:
  - live-state
  - ls-tui
milestone: m-26
dependencies: []
priority: low
type: bug
ordinal: 957000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The header line's `▲ N` (sessions with an ambiguous mux link) and the opt-in per-harness chips (`[tui] show_harness_chips`) count the session rows the view lists (`HeaderCounts::from_app` in `src/tui/ui/header.rs`), so in the PRs view a session listed under two pull requests of its checkout's branch counts twice. With a scratch copy of the showcase that adds a second open PR on `feature/extra`, the header reads `▲ 8` with or without a filter, over four ambiguous sessions (the showcase itself reads `▲ 4`). `N/M sessions` and the group headers' `(N)` and `▲` count each session once (CSP-879, CSP-856, ADR 0210's amendment); these two parts of the same line still count rows. Found while fixing CSP-879.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 In the PRs view a session listed under two pull requests counts once in the header line's ▲ N and in its harness chip
- [ ] #2 The reference frames are unchanged where each session is listed once
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
