---
id: CSP-077
title: Write the Atelier migration guide
status: Done
assignee: []
created_date: '2026-05-16 16:30'
labels:
  - p6
milestone: m-7
dependencies: []
ordinal: 72000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `docs/atelier-migration.md` mapping each overlapping
  Atelier command to its Conspectus replacement
  (`atelier session list` → `conspectus session`;
  `atelier mux status` → `conspectus session --projection mux`;
  forge status → `conspectus graph --format json` /
  `conspectus session`; graph-heavy parts of `atelier status` →
  `conspectus graph --format json`). Note the env toggles already
  documented in `docs/operations.md` and any new ones introduced by
  `CSP-076`. Link the migration guide from `docs/index.md`.
- Tests: docs-only; `git diff --check`.
- Manual checks: run the listed Conspectus commands and confirm
  they cover the workflow described.
- Blockers: none (independent of code changes).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `docs/atelier-migration.md` with mappings from
Atelier session, mux, forge, and graph-heavy status workflows to
Conspectus commands; documented read-only behavior, ambiguity
preservation, runtime knobs, and `conspectus::api` integration.
Linked the guide from `docs/index.md`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P6-005`
