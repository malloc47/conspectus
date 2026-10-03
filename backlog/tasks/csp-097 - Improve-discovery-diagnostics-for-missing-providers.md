---
id: CSP-097
title: Improve discovery diagnostics for missing providers
status: To Do
assignee: []
created_date: '2026-05-17 20:45'
labels:
  - h-obs
milestone: m-11
dependencies: []
ordinal: 115000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: when `gh` is unavailable, `tmux` is not installed, declared
  config is malformed, or a harness state root is missing, surface a
  `Diagnostic` row in the graph and a one-line stderr hint in CLI
  commands. Today some of these degrade silently (forge unavailable,
  missing state roots) while others (`ConfigDiagnostic`) only print to
  stderr.
- Tests: CLI integration tests that capture stderr and JSON
  diagnostics across each provider failure mode.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-OBS-005`
