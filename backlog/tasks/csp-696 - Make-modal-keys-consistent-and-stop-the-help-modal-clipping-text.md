---
id: CSP-696
title: Make modal keys consistent and stop the help modal clipping text
status: Done
assignee: []
created_date: '2026-10-24 01:44'
updated_date: '2026-10-24 02:18'
labels:
  - tui
milestone: m-23
dependencies: []
priority: medium
type: bug
ordinal: 772000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The Controls (filter) modal uses Enter to edit and ignores Space, while the newer launch and pin forms use Enter to submit, Esc to cancel, and Space to toggle a checkbox. The conventions should be one set. Separately, some help modal text is cut off at the modal edge instead of wrapping.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Space toggles the focused checkbox in every modal that has checkboxes, including Controls
- [x] #2 Enter and Esc mean the same thing in every modal (apply or submit, cancel or close), and a test enumerates all modals against one key convention table
- [x] #3 No help modal line is clipped at any supported width: long lines wrap or the page scrolls, asserted for every help page at several widths
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Controls honors Space; a modal key convention table is checked against every modal; help and menu text wrap instead of clipping, with word-integrity tests.
<!-- SECTION:FINAL_SUMMARY:END -->
