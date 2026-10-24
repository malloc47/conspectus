---
id: CSP-698
title: Keep hook observations across daemon refreshes
status: Done
assignee:
  - '@claude'
created_date: '2026-10-24 03:12'
updated_date: '2026-10-24 03:20'
labels:
  - hooks
milestone: m-20
dependencies: []
priority: high
type: bug
ordinal: 774000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
With `conspectus serve` running, `conspectus hook write` hands each record to the daemon's `hook_ingest` handler. That handler adds the record to the in-memory graph only. On every refresh, the hook-sidecar pass is evicted and rebuilt from `hooks-latest.json`, which nothing writes while the daemon is up. So a hook observation lasts only until the next cycle, a few seconds at most. In practice, Claude Code, Codex, and Pi hooks have had no lasting effect whenever the daemon runs, and sessions fall back to process-tree matching. On the operator's machine the spool was last written on 2026-10-02, and a live session showed up as a candidate in two tmux sessions instead of being tied to its own pane.

Observed by replaying a `SessionStart` payload: the `hook_sidecar` `linked_to_mux` link appeared right away and was gone within 15 seconds. The docs say the daemon persists hook evidence through `graph.bin`, but the refresh never carries it forward.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A hook record ingested by a running daemon is still attributed (its `hook_sidecar` link is present) after the daemon runs further refresh cycles
- [x] #2 A hook record ingested by a running daemon survives a daemon restart
- [x] #3 The daemonless path is unchanged: with no daemon, `hook write` still records the observation and the next in-process discovery applies it
- [x] #4 Tests cover ingest followed by a class refresh, and the daemon writes only where ADR 0143 allows
- [x] #5 Operator and design docs describe where hook observations persist with and without the daemon
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. In `handle_hook_ingest`, under the writer lock and after the snapshot check, write the record to the spool root the daemon's discovery config reads (`hook_sidecar_root`), then apply it to the live graph. If the spool write fails, return an error so the client falls back to writing the spool itself. ADR 0143 category 2 already lets the daemon write hook sidecars, so no new write path.
2. Tests: ingest, then a mux refresh keeps the `hook_sidecar` link; a fresh daemon context over the same spool attributes the record (restart); ingest with no spool root still works; the existing daemonless `hook write` tests keep passing.
3. New ADR recording that the spool is the durable hook record with or without a daemon, and that `graph.bin` alone can't carry hook evidence because refreshes rebuild the hook pass from the spool. Update design.md, operations.md, mux-link-resolution.md, and the ADR index.
4. `just check`, plus a manual replay against the live daemon.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Root cause: `handle_hook_ingest` changed only the in-memory snapshot. `hook_sidecar` is an always-rerun mutator, so every class cycle evicted its links and replayed `hooks-latest.json`, which nothing wrote while the daemon ran. Confirmed live: a replayed SessionStart link was gone within 15 s.
Validation: the new server tests (`an_ingested_hook_record_outlives_later_refreshes`, `an_ingested_hook_record_survives_a_daemon_restart`, `a_failed_spool_write_refuses_the_ingest_and_leaves_the_graph_alone`) fail without the fix and pass with it. The no-daemon path is covered by the unchanged `cli_smoke` `hook_write_*` tests and `testing_replay::replay_writes_hook_spool_records_into_discovery_pipeline`. `just check` passed (2878 tests).
The live daemon was not restarted to check the fix by hand; the running `conspectus serve` still has the old code until the operator restarts it.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The daemon `hook_ingest` handler now writes each record to the hook spool its own discovery reads, under the writer lock, before applying the record to the live graph. A failed spool write returns `hook_spool_write_failed`, and the client falls back to writing the spool itself. Hook attribution now lasts across refreshes and daemon restarts. Before, the next refresh (within about 5 s) dropped every ingested record, so hooks did nothing lasting whenever `conspectus serve` ran.
Recorded in ADR 0146, which refines ADR 0028 and stays inside ADR 0143 category 2. Updated design.md, operations.md, mux-link-resolution.md, the ADR index, and CHANGELOG.
Verified by three new server tests that fail without the fix, plus a passing `just check` (2878 tests).
<!-- SECTION:FINAL_SUMMARY:END -->
