---
id: CSP-667
title: Project history into the graph and audit live assumptions
status: Done
assignee:
  - '@claude'
created_date: '2026-10-22 01:11'
updated_date: '2026-10-22 12:46'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-664
  - CSP-665
  - CSP-666
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: high
type: feature
ordinal: 725000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Gone entities have to be graph nodes so the detail pane, Reveal, search, exports, and the new views can work with them. Much of the code assumes every node exists now. Attaching to a ghost mux or counting one as running would be a visible bug, so this ticket includes the audit.

- Scope: a sparse `node_presence` map (Live by default, Gone with last seen, Unknown); a history provider that emits gone nodes; `mux_last_hosted` and `rebuilt_from` relations; a `Cached` `linked_to_mux` fallback from a live mux's latest hosted session (ADR 0102's stickiness, for every mux); snapshot format version bump; presence in JSON export; audit of attach, rename, teardown, mux chips, "float muxed to top", header counts, the process-tree and hook linkers, worktree actions, and Reveal, each skipping or refusing gone nodes; presence-aware helpers on `SnapshotIndex`.
- Tests: resolver and model tests for each projection; tests that each audited action refuses or skips a gone node; fixtures with gone nodes in snapshot tests.
- Manual checks: with ghosts present, confirm the header mux count, attach, and "float muxed to top" ignore them.
- Blockers: the mux, checkout, and PR recording tickets.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 History records that are gone appear as graph nodes with presence Gone, and unprobed ones as Unknown
- [x] #2 A ghost mux links to its last session through `mux_last_hosted` and has no live-only relations
- [x] #3 A live mux's previous session holds the binding as the weakest evidence when current evidence drifts
- [x] #4 No action or count treats a gone node as live, with a test per audited consumer
- [x] #5 The snapshot format version is bumped
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Projection: node_presence, gone mux/checkout/repo nodes, mux_last_hosted, rebuilt_from, Cached linked_to_mux fallback; snapshot format 8.
2. Presence helpers on GraphSnapshot/SnapshotIndex; audit consumers and add a test per consumer.
<!-- SECTION:PLAN:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `history::project` and `GraphSnapshot::{node_presence, mux_probes, history}` (snapshot format 8) with `presence`, `is_live`, and `live_nodes` helpers. Records that aren't live become Gone or Unknown nodes with no live-only relations; a ghost links to its last session by `mux_last_hosted`; a rebuilt checkout links by `rebuilt_from`; each live mux's last hosted session holds its attribution as a `Cached` `linked_to_mux` that yields to current evidence (resolver-level tests cover both). Audited and fixed: the hook and process-tree linkers, observed-cwd git probes, table and TUI mux lists, header counts, attach (new `MuxGone` reason), rename (`MuxGone`), close-down targets, the worktree menu, and previews; `node show` and the DOT export show presence. One test per consumer in `src/history/audit_tests.rs` and the consumers' own test modules. Remaining consumers (pin float-to-top, ghost rows in views) belong to CSP-681/682.
<!-- SECTION:FINAL_SUMMARY:END -->
