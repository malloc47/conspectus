---
id: CSP-528
title: Numeric-suffix auto-increment for derived pin names
status: Done
assignee: []
created_date: '2026-09-28 15:11'
labels:
  - h-pin-tui
milestone: m-11
dependencies: []
ordinal: 330000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Motivation: a common workflow is spinning up `worker-2` from an
  already-pinned `worker-1` selection. The current derived-name
  helper (`unique_pin_mux_name`) treats the whole base as opaque
  and appends `-2`, `-3`, ... on collision, so a selection whose
  name already ends in a number becomes `worker-1-2` instead of
  `worker-2`. Operators then have to retype the primary name to
  get the intuitive next-in-series value.
- Scope: when the base name for a fresh pin ends in one or more
  trailing digits, bump the trailing number and pick the next
  variant that is free across live mux names, pinned mux names,
  and pin ids. When the base has no trailing digits, keep the
  existing `-2` / `-3` suffix behavior so unrelated call sites do
  not shift. Apply the derived name to the primary `display_name`,
  the derived pin `id`, and the derived mux `mux_name` together so
  the form opens with a self-consistent starting point. Cover the
  three natural entry points: selecting a pin row, selecting the
  agent-session row bound to a pin, and selecting the mux-session
  row bound to a pin.
- Tests: unit tests for the numbered-name helper (base without
  digits, base ending in digits, gap in the numbered sequence,
  already-free base); reducer / context tests for pin/agent/mux
  row selections whose derived name ends in a number; regression
  coverage that the existing `work` → `work-2` behavior is
  preserved for bases without a trailing number.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): landed in `c6905bb`.

The create form pre-fills the next-in-series name when
it starts from a numbered pin, so `worker-1` → `worker-2` (or
the lowest free integer above it) with no manual typing.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-011`
