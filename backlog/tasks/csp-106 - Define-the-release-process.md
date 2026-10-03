---
id: CSP-106
title: Define the release process
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-dist
milestone: m-11
dependencies:
  - CSP-103
  - CSP-104
ordinal: 139000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: ADR 0016 names the validation steps but the repo has no
  `CHANGELOG.md`, no release script, and no tagged-build workflow.
  Decide whether to adopt `cargo release` or a hand-rolled checklist
  and document it under `docs/operations.md` (or a new
  `docs/releasing.md`).
- Tests: dry-run the release procedure end-to-end before tagging
  `v0.1.0`.
- Blockers: `CSP-103`, `CSP-104`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-DIST-004`
