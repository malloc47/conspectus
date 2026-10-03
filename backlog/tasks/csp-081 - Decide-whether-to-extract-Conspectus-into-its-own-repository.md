---
id: CSP-081
title: Decide whether to extract Conspectus into its own repository
status: Done
assignee: []
created_date: '2026-05-16 16:30'
labels:
  - p6
milestone: m-7
dependencies:
  - CSP-073
  - CSP-079
ordinal: 76000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: per migration-plan item 7, reassess whether Conspectus
  should remain in this repository alongside its design ancestor
  or move to a standalone repo now that the shared library surface
  is stable. Record the conclusion in an ADR (and either schedule
  the extraction as a Phase 7 task or note that the current
  arrangement stays).
- Tests: docs-only.
- Manual checks: review the ADR against `docs/design.md` and
  `docs/naming.md`.
- Blockers: `CSP-073`, `CSP-079`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Accepted ADR 0017, which keeps Conspectus in the current
standalone repository, does not schedule a Phase 7 repository move,
and directs Atelier integration to use ADR 0016 distribution
channels rather than repository colocation.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P6-009`
