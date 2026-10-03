---
id: CSP-574
title: One-to-one pin bindings (ADR 0102)
status: Done
assignee: []
created_date: '2026-10-01 18:04'
labels:
  - h-pin-fix
milestone: m-11
dependencies: []
ordinal: 342000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: assign sessions to pins one-to-one, ranked by the
  resolver's session ↔ mux comparator; replace the previous pass's
  synthesized pin links with `Cached` fallback candidates;
  `PinStaleMux.claimed_elsewhere`; attach instead of `send-keys`
  when the stale pane already runs the pin's harness.
- Tests: `resolve::pins` assignment, fallback, heal-on-re-resolve,
  prior-binding hysteresis and recreated-mux cases,
  `mux_hosts_harness`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`apply_pin_bindings` ranks per pin with
`compare_session_mux` and assigns greedily across pins.
Snapshot format version 3. The sessions view's Pins group no
longer shows one session twice; `pin show` names the pin that
holds a contested session.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-FIX-001`
