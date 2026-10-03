---
id: CSP-039
title: Implement session-to-mux resolver scoring
status: Done
assignee: []
created_date: '2026-05-15 20:50'
labels:
  - p3
milestone: m-4
dependencies:
  - CSP-038
ordinal: 39000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: apply ADR 0006 scoring for session-to-mux candidates: local
  declared, global declared, strong process or provider evidence, exact
  cwd/root match, naming convention, then recency or activity correlation.
- Tests: table-driven resolver tests for each scoring tier, ties, ambiguity
  diagnostics, ignored candidates, and overridden candidates.
- Manual checks: inspect resolved relationships for one-to-many mux scenarios
  and confirm lower-ranked candidates remain visible.
- Blockers: `CSP-038`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Extended `MuxSessionNode` with optional `activity_epoch` and
`created_epoch`, forwarded the activity through `cross_link::infer`
onto `LinkedToMux` candidate metadata, and added a session-mux-specific
resolver comparator that ranks declared > strong > exact-cwd >
naming-convention > cached and breaks remaining ties by activity
recency. Ignored and overridden candidates continue to be skipped and
every losing candidate is recorded as a competing link plus a
`Conflict` diagnostic.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P3-008`
