---
id: CSP-350
title: Surface blocked-until state in CLI and TUI
status: To Do
assignee: []
created_date: '2026-06-03 12:44'
labels:
  - h-continue
milestone: m-11
dependencies:
  - CSP-349
ordinal: 228000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add opt-in table columns such as `blocked` /
  `resume-after`, a TUI row badge or detail-pane field, and
  `node show` output that displays the parsed resume time and source
  snippet. The default table columns should not grow unless the ADR
  explicitly changes the privacy/width posture established by ADR
  0023.
- Tests: table renderer snapshots, TUI buffer snapshots for blocked
  and non-blocked sessions, and `node show` output coverage.
- Blockers: `CSP-349`.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-CONTINUE-004`
