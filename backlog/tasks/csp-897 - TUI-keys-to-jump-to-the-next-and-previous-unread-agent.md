---
id: CSP-897
title: 'TUI: keys to jump to the next and previous unread agent'
status: Done
assignee:
  - '@claude'
created_date: '2026-11-17 23:53'
updated_date: '2026-11-18 00:40'
labels:
  - tui
dependencies: []
ordinal: 973000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The unread marks (ADR 0245) tell the operator what to read, but walking to each one means scrolling the tree. W already jumps to the next waiting agent (ADR 0216); the operator asked for the same for unread stops, in both directions. Their first idea, J and K, is taken by the right pane's preview scroll.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A key jumps to the next agent with an unread stop and another to the previous, wrapping, oldest unread stop first, from either pane and any view
- [x] #2 The cursor lands on the row standing for the agent in the current view (a mux's row in Mux) with the groups above it opened, else in the view a reveal lands in, as W does
- [x] #3 The status line says which unread stop it reached ('2 of 4 unread, since 1h ago') or why it stayed; the status bar and help list the keys from the key table only while something is unread
- [x] #4 J and K keep scrolling the preview; no existing key changes
- [x] #5 docs/tui-columns.md, docs/operations.md, and an ADR record the keys and their order; tests cover both directions, wrapping, the Mux view, a switch of view, and nothing unread
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`}` goes to the next agent with an unread stop and `{` to the previous, oldest stop first, wrapping, from either pane and every view, as `W` does for waits (`App::jump_along`, `Walk`). Unlike `W` they stay in the view on screen while another unread agent has a row there (in Mux, the muxes) and switch views only when none does (ADR 0247). The status line says `2 of 4 unread, since 1h ago` or why the cursor stayed; both bindings are available only while a stop is unread, so the status bar, help page, and legend list them from the key table. J, K, [ and ] are unchanged. Verified: ten consistency tests (both directions, wrapping, the Mux view staying put, a switch and Backspace back, nothing unread, only one unread, filters, groups opened, status bar and help), the key-table tests, and the full pre-main gate.
<!-- SECTION:FINAL_SUMMARY:END -->
