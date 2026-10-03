---
id: CSP-135
title: 'ADR: process-tree linker design and dependency choice'
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - h-muxproc
milestone: m-11
dependencies: []
ordinal: 244000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: decide (1) whether to depend on the `sysinfo` crate or
  read `/proc` directly on Linux and an equivalent on macOS (and
  whether macOS is in scope at all for the first pass), (2) the
  descendant-depth bound and the pane-PID acquisition path
  (`tmux list-panes -F "#{pane_id} #{pane_pid}"`), (3) the
  known-binary match set and how it extends as new harnesses are
  added, (4) confidence and provenance assignment for the emitted
  `AgentInPane` evidence (likely `Discovered` provenance, `High`
  confidence for a direct command-name match, demoted for the
  fallback heuristics), and (5) where the linker fits in the module
  layout (peer of `discovery/tmux/`, or a sub-module that consumes
  the existing tmux runner). Record as a new ADR per CLAUDE.md.
- Tests: none directly; ADR is the deliverable.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0046 chooses a Linux-first, dependency-free `/proc`
reader with an injectable process snapshot seam, a four-edge
descendant walk from tmux active-pane PID, the supported harness
binary match set, `active_pane_process_match` evidence, and
`CONSPECTUS_DISABLE_PROCTREE` as the runtime kill switch.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-001`
