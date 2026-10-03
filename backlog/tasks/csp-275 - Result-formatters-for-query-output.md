---
id: CSP-275
title: Result formatters for query output
status: Done
assignee: []
created_date: '2026-05-25 21:41'
labels:
  - p9
milestone: m-14
dependencies:
  - CSP-274
ordinal: 467000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: support `--format table` (default, columnar, width-aware
  via the existing renderer's truncation helpers), `--format json`
  (one object per row), `--format csv`, `--format tsv`. Width-aware
  output mirrors the existing `conspectus table` behavior
  (ADR 0020). Color codes per ADR 0022 when stdout is a TTY.
- Tests: format snapshots over fixture queries; width-aware
  truncation snapshots at 80 and 160 columns.
- Manual checks: pipe each format to a file and inspect.
- Blockers: `CSP-274`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Query output supports `--format table|json|csv|tsv`.
Table output reuses the shared width-aware rendering substrate and
honors `--width`, `--wide`, and `--color`; CLI and runner tests
cover each format and truncation behavior.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P9-005`
