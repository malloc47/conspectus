---
id: CSP-160
title: Lock v1 TUI product decisions (operator-journey core)
status: Done
assignee: []
created_date: '2026-05-19 03:51'
labels:
  - p8
milestone: m-13
dependencies: []
ordinal: 386000
---

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The implementation doc records the primary persona
(Returning Operator), the v1 default view (`sessions`),
configuration knobs for default view and sort, hierarchy-first
sort default, 30 s / 2 s refresh defaults, agent-deck-style
direct-row-key action UX (no modal picker, no command palette),
right-panel header+preview composition (no tabs), in-process
polling for v1 with Phase 7 server mode reserved, and the
`Enter` / `a` / `R` semantics. The remaining v1-blocking
decisions (project grouping, mux target granularity, ambiguous
mux-link behavior, PR detail depth) move to CSP-160.01; the
v1-deferrable questions move to a "Locked v1 Decisions" /
"Open Product Questions (v1-deferrable)" section.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P8-001`
