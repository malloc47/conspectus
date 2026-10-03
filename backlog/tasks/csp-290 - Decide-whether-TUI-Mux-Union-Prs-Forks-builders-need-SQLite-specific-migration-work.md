---
id: CSP-290
title: >-
  Decide whether TUI Mux/Union/Prs/Forks builders need SQLite-specific migration
  work
status: Done
assignee: []
created_date: '2026-05-26 23:06'
labels:
  - p10
milestone: m-15
dependencies: []
ordinal: 483000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: intentionally skip this during Phase 10 closeout. The
  corresponding ADR 0031 TUI row builders have not landed yet, so
  there is no active in-memory consumer to migrate. Revisit after
  `CSP-291` / `CSP-292` settle the consumer-side contract and decide
  whether the future Mux/Union/Prs/Forks interfaces need any
  P10-specific context or can be built directly on the post-P10
  `Connection` surface.
- Blockers: ADR 0031 stories that introduce the relevant builders.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
No Phase 10 migration work is needed for these builders.
Future TUI Mux/Union/Prs/Forks row builders should be implemented
directly against the post-P10 `Connection` surface instead of
adding snapshot-first builders and migrating them later.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P10-012`
