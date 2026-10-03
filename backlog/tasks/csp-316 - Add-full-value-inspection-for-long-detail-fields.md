---
id: CSP-316
title: Add full-value inspection for long detail fields
status: Done
assignee: []
created_date: '2026-06-01 00:28'
labels:
  - t8
milestone: m-13
dependencies:
  - CSP-315
ordinal: 420000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: provide a focused way to inspect long values from the core
  summary, relationship rows, and previews without forcing them into
  the main detail layout. Candidate UX: `o` opens a centered
  read-only value modal for the selected field/row, with wrapping,
  scroll, and copy-oriented text. Reuse existing modal/input
  primitives where possible and avoid new dependencies.
- Tests: widget/reducer tests for opening, scrolling, and closing the
  full-value view; buffer snapshots for long command and observation
  key values.
- Blockers: `CSP-315`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `T8-030`
