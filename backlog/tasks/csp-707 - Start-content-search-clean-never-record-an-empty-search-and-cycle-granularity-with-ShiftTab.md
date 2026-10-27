---
id: CSP-707
title: >-
  Start content search clean, never record an empty search, and cycle
  granularity with Shift+Tab
status: Done
assignee: []
created_date: '2026-10-27 02:04'
updated_date: '2026-10-27 02:51'
labels:
  - h-search
  - tui
milestone: m-11
dependencies: []
priority: medium
type: bug
ordinal: 793000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Follow-up from hands-on testing: an empty search string must never become a history entry, opening content search again must start with an empty string (a key pages back through earlier search strings to bring one back), and Shift+Tab should cycle between Sessions and Messages in the content modal.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 No path records a history entry whose query is empty or only whitespace
- [x] #2 Ctrl+/ and s always open content search with an empty query; a documented key pages backwards (and forwards) through earlier search strings, filling the field and searching, without duplicating history
- [x] #3 In the content modal Tab switches Current/All and Shift+Tab cycles Sessions/Messages; the footer, help, docs, and tests say so
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Empty queries are never recorded; content search opens clean on All; Ctrl-R and Ctrl-T page earlier search strings; Shift-Tab cycles Sessions and Messages.
<!-- SECTION:FINAL_SUMMARY:END -->
