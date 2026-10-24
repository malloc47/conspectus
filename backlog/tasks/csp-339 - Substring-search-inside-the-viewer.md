---
id: CSP-339
title: Substring search inside the viewer
status: Done
assignee: []
created_date: '2026-06-02 18:59'
updated_date: '2026-10-24 04:47'
labels:
  - h-viewer-native
milestone: m-11
dependencies:
  - CSP-338
ordinal: 214000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: `/` opens a search prompt at the footer. `n` / `N`
  cycle matches. Matches highlight in the body. Search is
  case-insensitive substring over rendered turn bodies (no
  fuzzy index in v1, matching ADR 0024).
- Tests: snapshot tests for search-open, match-highlight,
  no-match cases.
- Blockers: `CSP-338`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-VIEWER-NATIVE-007`

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Superseded by CSP-616.05 (find inside the viewer), which also adds open-at-match and reuses the shared matcher.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Closed as a duplicate of CSP-616.05; no separate work.
<!-- SECTION:FINAL_SUMMARY:END -->
