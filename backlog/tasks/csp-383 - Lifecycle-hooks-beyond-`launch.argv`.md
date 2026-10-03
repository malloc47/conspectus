---
id: CSP-383
title: Lifecycle hooks beyond `launch.argv`
status: To Do
assignee: []
created_date: '2026-06-04 21:32'
labels:
  - h-pin-f
milestone: m-11
dependencies: []
ordinal: 332000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- When a concrete pattern emerges that prefix tooling
  (`nix develop --command`, `direnv exec`, `op run`) cannot
  express cleanly, grow a `launch.before` / `launch.after`
  surface (or `[[pins.hooks]]`). v1 schema is forward-compatible
  with such an addition.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-PIN-F-002`
