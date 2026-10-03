---
id: CSP-504
title: Add better recency sort options for the mux view
status: Done
assignee: []
created_date: '2026-07-28 03:30'
labels:
  - h-mux-sort
milestone: m-18
dependencies: []
ordinal: 529000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: the mux view currently sorts by the shared `Sort::Recency`
  signal. Enumerate the recency signals tmux exposes per session
  (`session_created`, `session_activity`, `session_last_attached`,
  and the active pane's process start time via the pane PID) and,
  where a signal is reliably available, expose it as a selectable mux
  sort option. Fold the chosen signal(s) into the `MuxSessionRow`
  model and the sort/controls surface. Fall back gracefully when a
  signal is missing.
- Tests: parser tests for the added `tmux` format fields; sort tests
  over fixtures exercising each signal and the missing-signal
  fallback.
- Blockers: depends on which signals survive the tmux capability
  survey (part of the story) and an operator call on which become
  selectable options.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Chose the mux-scoped sub-option approach. tmux survey
found `session_activity` and `session_created` already captured;
added `session_last_attached` (new `MuxSessionNode.last_attached_epoch`
+ format field). New `MuxRecency { Activity, Created, LastAttached }`
basis selects the epoch the mux view's `Sort::Recency` orders by;
exposed as a mux-only "Recency by" section in the controls overlay
(menu-first), persisted in `tui-state.json`, and defaulting to
Activity (the prior behavior). Pane-process start time was surveyed
and dropped (Linux-only, extra syscalls, ~duplicates activity).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUX-SORT-001`
