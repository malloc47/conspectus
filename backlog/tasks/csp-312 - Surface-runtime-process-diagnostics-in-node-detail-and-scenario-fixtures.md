---
id: CSP-312
title: Surface runtime process diagnostics in node detail and scenario fixtures
status: Done
assignee: []
created_date: '2026-05-31 19:22'
labels:
  - h-muxproc-fu
milestone: m-11
dependencies:
  - CSP-311
  - CSP-306
ordinal: 254000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add node-detail sections for runtime process nodes and for
  agent/mux nodes linked through process evidence. Extend named dev
  scenarios so process-cardinality and stale-argv cases can be
  inspected through `dev scenario graph/table/node/tui`.
- Tests: node-show/detail snapshots and dev-scenario coverage for
  process-backed attribution cases.
- Blockers: `CSP-311`, `CSP-306`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
TUI/node detail now has a `Process` section for runtime
process fields and linked process context from agent and mux
details. Runtime process details link back to containing muxes and
identified/candidate sessions, annotating candidate session links.
Added a named `process-cardinality` dev scenario with two runtime
process observations for one mux; `codex-fd-current` continues to
cover stale argv vs fd-backed process attribution.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-FU-006`
