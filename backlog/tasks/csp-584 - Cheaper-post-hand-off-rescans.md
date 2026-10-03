---
id: CSP-584
title: Cheaper post-hand-off rescans
status: To Do
assignee: []
created_date: '2026-10-02 23:54'
labels:
  - h-handoff-latency
milestone: m-18
dependencies: []
ordinal: 552000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: one daemon `refresh` call that takes several classes (one
  resolve and one round trip instead of two). Measure how long nudges
  wait on the writer lock during git rebuilds. Decide whether the
  blocking refreshes before a launch's attach (`pin launch`,
  `mux new`, `mux launch`) are still needed, since the attach target
  comes from the request, not the snapshot.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-HANDOFF-LATENCY-002`
