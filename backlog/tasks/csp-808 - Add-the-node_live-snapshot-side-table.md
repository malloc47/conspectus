---
id: CSP-808
title: Add the node_live snapshot side table
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 03:03'
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
- [x] #1 `GraphSnapshot` carries `node_live`; a snapshot with an empty table serializes exactly as before
- [x] #2 `FORMAT_VERSION` is bumped and an old `graph.bin` triggers the documented cold rebuild
- [x] #3 `live_state_of` and `live_counts_of_mux` return effective states and per-state counts, including pane-only entries on a mux
- [x] #4 A change to `node_live` alone does not change the process-tree fingerprint
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Build a baseline `--features snapshot` binary and render several showcase-fixture TUI frames for the before/after comparison.
2. Add `node_live: BTreeMap<NodeId, LiveObservation>` to `GraphSnapshot` after `node_presence`, with a `node_live_serde` list helper (node_id plus the flattened observation), omitted from JSON when empty; rkyv derives come with the struct. Bump `FORMAT_VERSION` 9 -> 10.
3. `evict_provider` drops entries for evicted nodes. `canonicalize` needs no sort (BTreeMap), so document that. `From<GraphSnapshot> for GraphFragment` destructures the snapshot so every field it leaves behind is named (a new field fails to compile until decided); `GraphFragment` stays provider output and gains no field (it is public library API with full literals in every provider). `merge_with_prior` keeps the fresh table and drops the prior one, because the live-state mutator rebuilds it each cycle (ADR 0205 decision 7).
4. Accessors on `GraphSnapshot`: `live_state_of(id, now_ms, liveness)` and `live_counts_of_mux(mux, now_ms, liveness)`, where `liveness` is a closure from the stored observation to `Liveness`; counts cover sessions whose `preferred_mux_for` is the mux plus the mux's own pane-only entry.
5. Tests in a new `src/model/graph_tests.rs`: empty table JSON unchanged, populated JSON and rkyv round trips, eviction, canonicalize, accessors (mux with 0, 1, many sessions, pane-only entry, session without entry, liveness and time rules). Fingerprint test in `discovery_tests.rs`; merge test there too. Old-version graph.bin cold rebuild test in `tests/cli_persist.rs` plus a header test in `snapshot_tests.rs`. New JSON golden `graph_snapshots__live_state`. Add the table to the rkyv archive fixture in `model_tests.rs`.
6. Docs: library-api.md, CHANGELOG.md, design.md if needed. Run checks.sh and run-tests.sh; compare TUI frames against the baseline; finalize the task.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decisions: `GraphFragment` gains no field. It is public provider output with full literals in every provider, and nothing emits live state through a fragment; `From<GraphSnapshot>` now destructures the snapshot so each field it leaves behind is named and a new field fails to compile until placed. `merge_with_prior` keeps the fresh table and drops the prior's (ADR 0205 decision 7: the mutator rebuilds it every cycle). `canonicalize` needs no change for a `BTreeMap`; its doc now says so. Both accessors take the caller's liveness as a closure from the stored observation (called only when an entry exists); ADR 0205 decision 5 now records that.
Validation: checks.sh (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check) passed. run-tests.sh: 4977 passed, 1 failed (the known `discovery::forge` "failed to spawn gh binary" flake; passed alone on rerun), 1 ignored. Existing graph goldens unchanged; new `graph_snapshots__live_state` golden. `conspectus tui --snapshot` frames for 9 showcase views (sessions, transcript, related preview, repo, mux, mux pane, PRs, pinned, history) were byte-identical between a baseline binary built at 2e31f378 and this change.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the sparse `node_live: BTreeMap<NodeId, LiveObservation>` table to `GraphSnapshot` (ADR 0205). It serializes to JSON as a list of `node_id` plus the observation's fields, omitted when empty, rides in graph.bin through rkyv, and bumps `FORMAT_VERSION` from 9 to 10. Accessors `live_state_of(id, now_ms, liveness)` and `live_counts_of_mux(mux, now_ms, liveness)` read effective states; the counts cover sessions whose preferred mux is the mux plus its pane-only entry, and skip sessions with no entry. `evict_provider` drops entries of evicted nodes, `merge_with_prior` keeps fresh's table and drops the prior's, the `GraphFragment` conversion names every field it leaves behind, and the process-tree fingerprint never sees the table. No producer and no TUI change.

Verified: new unit tests in `src/model/graph_tests.rs` (JSON and rkyv round trips, empty table unchanged, unknown names, eviction, canonicalize, accessors with 0, 1, many sessions, pane-only entries, sessions without entries, liveness and time rules), the archive fixture in `model_tests.rs`, fingerprint, merge, and fragment tests in `discovery_tests.rs`, the old-format refusal in `snapshot_tests.rs`, and cold-rebuild tests for `refresh --class` (`tests/cli_persist.rs`) and the daemon (`tests/cli_serve.rs`). Existing goldens unchanged; new `graph_snapshots__live_state` golden. checks.sh passed; run-tests.sh had 4977 passing with only the known `discovery::forge` flake, which passed on rerun. TUI frames for 9 showcase views were byte-identical before and after. Docs: CHANGELOG, `docs/library-api.md`, and ADR 0205 decisions 5 and 7.
<!-- SECTION:FINAL_SUMMARY:END -->
