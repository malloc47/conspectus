---
id: CSP-310
title: Emit runtime process nodes from MUXPROC discovery
status: Done
assignee: []
created_date: '2026-05-31 19:22'
labels:
  - h-muxproc-fu
milestone: m-11
dependencies:
  - CSP-309
ordinal: 252000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: update process-tree, fd, hook/plugin, and Codex log-derived
  attribution paths to emit process observations and explicit
  process/session evidence instead of hiding all process facts inside
  `LinkedToMux.source_metadata`. Preserve compatibility metadata only
  where needed during migration.
- Tests: fixture-backed process-tree tests for direct/nested matches,
  no matching session, ambiguous same-cwd sessions, subagent roles,
  stale argv suppressed by stronger current-session evidence, and
  unreadable `/proc` degradation.
- Blockers: `CSP-309`.
- **slice landed**: process-tree evidence now emits
  `RuntimeProcess` nodes, `mux_contains_process` links, and
  `process_identifies_session` / `process_candidates_session`
  links alongside the legacy `linked_to_mux` candidates. Hook
  sidecar records with process ids and Codex log-derived pid/thread
  attribution emit the same process graph shape. The ERD in
  `docs/design.md` now includes mux/process/session relationships.
  Active-pane fd evidence without a process-tree snapshot now
  synthesizes a root process observation from the mux active pane PID,
  preserving the runtime process graph shape for deterministic
  scenarios and constrained platforms.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-MUXPROC-FU-004`
