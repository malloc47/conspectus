---
id: CSP-258
title: 'CLI flag parity: shared `FilterArgs` + per-view grouping'
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies:
  - CSP-250
ordinal: 454000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: introduce a shared `FilterArgs` struct mounted on both
  `TuiArgs` and `TableArgs`, exposing `--harness` (repeatable),
  `--max-age <DURATION>`, `--mux-state` (comma-or-repeat), and
  `--grouping <VALUE>` (per-view; values depend on `--view`).
  Legacy `--sessions-grouping` stays as an alias that prints a
  one-line deprecation warning to stderr.
- Tests: CLI smoke tests for each flag, duration parse-error
  messages, deprecation-warning emission, `--grouping` rejected
  with a useful message when given an invalid value for the
  chosen view.
- Blockers: `CSP-250`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in `src/cli.rs` — `FilterArgs` mounted on
both `TuiArgs` and `TableArgs` exposes `--harness`,
`--max-age <DURATION>`, `--mux-state`, and `--grouping
<VALUE>` (per-view validation). `--sessions-grouping` stays
as a deprecated alias with a stderr warning. CLI smoke tests
cover each flag, duration parse errors, and view-scoped
grouping rejection messages. (Table-side consumption for the
non-sessions projections is the remaining CSP-259 work.)
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-009`
