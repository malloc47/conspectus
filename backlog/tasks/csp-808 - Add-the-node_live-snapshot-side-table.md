---
id: CSP-808
title: Add the node_live snapshot side table
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 02:27'
labels:
  - live-state
  - ls-model
milestone: m-26
dependencies:
  - CSP-807
documentation:
  - >-
    docs/adr/0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md
priority: high
type: enhancement
ordinal: 894000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Live state has to reach every consumer of a snapshot (TUI row builders, `table`, `graph --format json`, `node show`, the library) without making identity nodes carry fast-changing fields or reopening the process-tree fingerprint. ADR 0205 chooses a sparse side table next to `node_presence`.

Scope: Add `node_live: BTreeMap<NodeId, LiveObservation>` to `GraphSnapshot` with a serde helper that writes a list and omits it when empty (as `node_presence_serde` does), rkyv support, and the `FORMAT_VERSION` bump. Wire it into `canonicalize`, `evict_provider`, and the `GraphFragment` conversions so nothing drops it silently. Add accessors `live_state_of(id, now)` (the effective state) and `live_counts_of_mux(id, now)` (counts over the mux's hosted sessions found with the existing preferred-mux helpers, plus pane-only entries keyed on the mux). Update every full `GraphSnapshot` literal. The table is not part of the process-tree fingerprint.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Existing graph goldens are unchanged for snapshots with no live state. A new JSON golden shows the table. Round-trip through rkyv and JSON. A test shows a state change leaves the fingerprint identical. Eviction and merge tests.

Blockers: The vocabulary.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 `GraphSnapshot` carries `node_live`; a snapshot with an empty table serializes exactly as before
- [ ] #2 `FORMAT_VERSION` is bumped and an old `graph.bin` triggers the documented cold rebuild
- [ ] #3 `live_state_of` and `live_counts_of_mux` return effective states and per-state counts, including pane-only entries on a mux
- [ ] #4 A change to `node_live` alone does not change the process-tree fingerprint
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
