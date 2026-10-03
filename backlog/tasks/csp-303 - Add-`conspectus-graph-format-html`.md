---
id: CSP-303
title: Add `conspectus graph --format html`
status: Done
assignee: []
created_date: '2026-05-30 04:47'
labels:
  - gv
milestone: m-16
dependencies: []
ordinal: 490000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Split into `CSP-303.01` / `CSP-303.02` / `CSP-303.03` so the foundational payload and vendoring story are settled before the chrome is built on top. Aggregate scope is unchanged from the original ticket: a single-file self-contained HTML explorer backed by the same resolved graph as DOT, supporting pan/zoom, selection, neighbor highlighting, search/filter, an inspector, and the navigation primitives from ADR 0050 decision 10. Closed when CSP-303.01/CSP-303.02/CSP-303.03 all landed; CSP-303.04 (layout selector + dagre) shipped in parallel as a follow-up.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `GV-003`
