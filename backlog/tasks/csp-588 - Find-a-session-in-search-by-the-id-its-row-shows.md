---
id: CSP-588
title: Find a session in search by the id its row shows
status: Done
assignee:
  - '@claude'
created_date: '2026-10-12 13:55'
updated_date: '2026-10-12 18:06'
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
- [x] #1 Typing the start of a session key in the search overlay finds that session
- [x] #2 Queries that match today still match
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
Add the session key to the session row's search haystack beside the short id; test both ids match.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Search now matches a session by its key, the id its row's id column shows, as well as the content-addressed short id. Verified with `just check`, a unit test covering a key prefix, a full pasted key, and the short id, and a live snapshot where `/7e2c3973` selects that session.
<!-- SECTION:FINAL_SUMMARY:END -->
