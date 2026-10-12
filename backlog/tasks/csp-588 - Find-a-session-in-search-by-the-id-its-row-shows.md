---
id: CSP-588
title: Find a session in search by the id its row shows
status: To Do
assignee: []
created_date: '2026-10-12 13:55'
labels:
  - rel
milestone: m-20
dependencies: []
priority: medium
ordinal: 637000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Why: a session row shows the first characters of the session key (`11536dfb`), but search matches the content-addressed short id the status bar uses (`claude:661c9a`), so typing the id on screen finds nothing. Found while driving `tui --snapshot` by search for `CSP-585.02`.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Typing the start of a session key in the search overlay finds that session
- [ ] #2 Queries that match today still match
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
