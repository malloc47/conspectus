---
id: CSP-254
title: Accelerator keybindings + view-switching plumbing
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies:
  - CSP-251
  - CSP-253
ordinal: 450000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: bind `v` (controls overlay), `1`–`5` (direct view
  switch), `]`/`[` (cycle views), `f` (jump into the Filters
  section), `F` (clear all filters), and the grouping-cycle key.
  Resolve the `G` collision with End — either move "last row" to
  `End` only and reuse `G`, or bind grouping-cycle to `Ctrl-G`.
  Repurposes `f` from the previously-reserved fork action per
  ADR 0031; impl doc updated. Closes the `CSP-165` deferred view-
  switching slice.
- Tests: reducer tests for each new keybinding; Ratatui snapshots
  for the overlay open vs accelerator-only paths producing the
  same end state.
- Blockers: `CSP-251`, `CSP-253`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in `src/tui/runtime.rs` — `1`–`5` (direct
view), `]`/`[` (cycle), `f` (controls overlay), `F` (clear
filters), `Ctrl-G` (cycle grouping). `v` was reassigned to
the session viewer in CSP-340 and `f` took over
the controls overlay role originally planned for `v`. Reducer
tests pin each binding.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-005`
