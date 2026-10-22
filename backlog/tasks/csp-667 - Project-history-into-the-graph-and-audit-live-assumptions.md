---
id: CSP-667
title: Project history into the graph and audit live assumptions
status: To Do
assignee: []
created_date: '2026-10-22 01:11'
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
- [ ] #1 History records that are gone appear as graph nodes with presence Gone, and unprobed ones as Unknown
- [ ] #2 A ghost mux links to its last session through `mux_last_hosted` and has no live-only relations
- [ ] #3 A live mux's previous session holds the binding as the weakest evidence when current evidence drifts
- [ ] #4 No action or count treats a gone node as live, with a test per audited consumer
- [ ] #5 The snapshot format version is bumped
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
