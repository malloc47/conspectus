---
id: CSP-255
title: Multi-select list widget
status: Done
assignee: []
created_date: '2026-05-24 01:47'
labels:
  - f8
milestone: m-13
dependencies: []
ordinal: 451000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: shared list-with-checkbox primitive in
  `src/tui/widgets/multi_select.rs` for the harness and mux-state
  sub-editors. Pure state machine: cursor up/down, Space toggle,
  Enter confirms with `Vec<T>`, Esc cancels. Renders as a small
  bordered list anchored next to the originating row.
- Tests: unit tests for cursor wrap, toggle semantics, empty-
  commit (clears the predicate), and large-list scrolling.
- Blockers: none beyond ADR 0031; can land in parallel with
  `CSP-253`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Shipped in `src/tui/widgets/multi_select.rs`. Pure
state machine with cursor up/down, Space toggle, Enter
confirm, Esc cancel. Reused by both the harness and
mux-state sub-editors. Subsequently ported (CSP-437) to
sit on the upstream multi-select primitive while keeping the
same surface.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `F8-006`
