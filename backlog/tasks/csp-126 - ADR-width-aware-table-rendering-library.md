---
id: CSP-126
title: 'ADR: width-aware table rendering library'
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - h-tbl
milestone: m-11
dependencies: []
ordinal: 118000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0020 records the decision to roll our own minimal
width-aware renderer under `src/output/`, depending only on
`unicode-width` and `terminal_size`. `comfy-table` (upstream feature
freeze, wraps rather than truncates), `tabled` (heavier surface, API
churn risk for snapshot tests), `cli-table`, and `prettytable-rs`
were considered and rejected. Both dependencies are runtime deps
added in `CSP-128`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-TBL-001`
