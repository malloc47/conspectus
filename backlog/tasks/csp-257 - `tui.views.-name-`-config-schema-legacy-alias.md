---
id: CSP-257
title: '`[tui.views.<name>]` config schema + legacy alias'
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies: []
ordinal: 453000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: parse `[tui.views.<name>] grouping = "…"` and
  `[[tui.views.<name>.filters]]` sub-tables in `src/config.rs`.
  Existing `[tui].sessions_grouping` continues to load as a
  deprecated alias that emits a one-line warning to stderr when
  encountered and seeds
  `[tui.views.sessions].grouping` when the new key is absent.
  Multiple `[[…filters]]` entries OR their predicates (set
  union).
- Tests: loader unit tests for new schema, legacy alias alone,
  both present (new wins, warning emitted), malformed values,
  array-of-tables filter unions.
- Blockers: ADR 0031; independent of TUI work.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in `src/config.rs` — `TuiViewsConfig` parses
`[tui.views.<name>] grouping = "…"` and
`[[tui.views.<name>.filters]]` sub-tables. The legacy
`[tui].sessions_grouping` key emits a one-line deprecation
warning and seeds `[tui.views.sessions].grouping` only when
the new key is absent. Loader unit tests cover both schemas,
the alias path, conflict resolution, and malformed values.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-008`
