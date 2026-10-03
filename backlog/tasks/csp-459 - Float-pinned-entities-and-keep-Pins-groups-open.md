---
id: CSP-459
title: Float pinned entities and keep Pins groups open
status: Done
assignee: []
created_date: '2026-06-25 19:23'
labels:
  - h-pin-tui
milestone: m-11
dependencies:
  - CSP-453
ordinal: 327000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: in grouped session/mux views, keep the synthetic Pins
  group at the top and expand it by default even when the launch
  context is elsewhere. In flat session/mux views, float entities
  with resolved pin bindings directly to the top without adding a
  synthetic group header. Bound pins float their agent-session row;
  bound and stale pins float their mux row. Unbound pins remain
  represented by the synthetic pin row until `CSP-460`
  introduces first-class pin graph nodes.
- Tests: sessions and mux row-builder tests for grouped ordering,
  default expansion, flat pinned-row sort priority, stale-mux sort
  priority, and unbound behavior.
- Blockers: `CSP-453`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Synthetic Pins groups remain top-level and are expanded
by default when present. Flat sessions sort bound pinned session
rows before non-pinned sessions, and flat mux views sort muxes
targeted by bound/stale pins before ordinary muxes while keeping
the flat list free of a synthetic Pins header. Session and mux
detail panes now include a `pin` field for healthy bound/stale
pin associations, and selecting a bound/stale synthetic pin row
resolves the right pane to the realizing session or mux detail.
Fully unbound pins still require `CSP-460` because they do
not have graph node identity yet.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-TUI-008`
