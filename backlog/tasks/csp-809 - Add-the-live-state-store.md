---
id: CSP-809
title: Add the live-state store
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 03:50'
labels:
  - live-state
  - ls-store
milestone: m-26
dependencies:
  - CSP-807
documentation:
  - docs/adr/0204-the-live-state-store-folds-agent-events-per-session.md
  - docs/plans/live-state/research/06-internal-audit.md
priority: high
type: enhancement
ordinal: 895000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The attribution spool keeps one record per tmux session, with one-second timestamps, no lock, and no history, so it cannot hold per-session state: split-pane agents overwrite each other and events within a second land in any order. Stateless hooks (Claude Code, Codex) also need something that holds a session's state between events. ADR 0204 puts that in a separate store.

Scope: Add the store module (for example `src/live_store.rs`): the file `$XDG_STATE_HOME/conspectus/live-state.json` with `live-state.lock`, atomic writes under an advisory lock (copy the history store's helpers), the entry shape from ADR 0204, a store-wide revision counter, snapshot replacement for plugin sources by `(instance, seq)`, the one-live-entry-per-`(harness, pid)` supersede rule, garbage collection on write (exited after an hour, dead-pid after a day, 512 entries), a lock wait of 200 ms that drops the event, and tolerant readers (missing, unreadable, or newer-schema files mean no state). The internal `reducer` block is opaque to readers. Define the neutral `LiveEvent` type the writers produce and a `Reducer` trait with the Claude Code and Codex reducers as later stories.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Unit tests: ordering under concurrent writers (spawn threads or processes), lock timeout drops the event, supersede on a new session id from the same pid, snapshot ordering by `(instance, seq)` and by time for a new instance, GC, newer-schema and corrupt files, and a privacy test that plants sentinel strings in every payload-bearing input and asserts none reaches the file.

Blockers: The vocabulary.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 The store reads and writes `live-state.json` atomically under a lock and never blocks a writer more than 200 ms
- [x] #2 Entries are keyed by harness and session id and carry the tmux socket, session, and pane when known
- [x] #3 A new session id from the same pid marks the old entry `exited/ended`
- [x] #4 A reader treats any missing, corrupt, or newer-schema file as no live state
- [x] #5 No payload text can reach the file; a sentinel test enforces it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Add `src/live_store/` (doc-hidden like `hook`): `event.rs` (neutral `LiveEvent` with `LiveContext` and a closed `LiveEventKind`, plus `LiveSnapshot` for plugin sources), `reducer.rs` (the shared fold and a `Reducer` trait whose default is the shared fold), `file.rs` (`LiveFile`/`LiveEntry`, the pure apply, supersede, and garbage-collection rules, tolerant parsing), `store.rs` (`LiveStore`: path from `$XDG_STATE_HOME` as the history store resolves it, `live-state.lock` with a 200 ms try-lock then `Applied::Dropped`, atomic private writes through the declared helpers, store-wide `rev`), `process.rs` (pid start time and liveness from `/proc`).
2. Unit tests in sibling `_tests.rs` files: reducer sequences from the research tables (simple turn, permission cleared by tool end, interrupt, question, plan, parallel and background subagents, mid-turn compaction, clear superseding, rate limit, session end), snapshot ordering by `(instance, seq)` and by time, supersede, GC, missing/corrupt/newer-schema files, lock timeout, concurrent writers on threads, and a privacy test (sentinels plus a walk over every string the file can hold).
3. Refine ADR 0204 with the decisions taken (event set, supersede for event sources, exited entries, id checks), update docs/library-api.md internal list, verify capability tests need no entry, run checks and the full suite, compare `tui --snapshot` frames before and after.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Landed `src/live_store/` (doc-hidden like `hook`): `event.rs` (neutral `LiveEvent`/`LiveContext`/`LiveEventKind`, `LiveSnapshot`, id and path plausibility checks), `reducer.rs` (shared `fold`, `Reducer` trait with `SharedReducer`, private `ReducerBlock`), `file.rs` (`LiveFile`/`LiveEntry`, snapshot replacement, supersede, GC, tolerant parse), `store.rs` (`LiveStore::at`/`from_env`, 200 ms try-lock then `Applied::Dropped`, atomic private writes via the declared helpers, store-wide `rev`), `process.rs` (pid start time and liveness from `/proc`).
Decisions recorded in ADR 0204: the event set and one shared reducer; supersede applies to event sources only (a snapshot source can host several sessions per process); a pid whose start time changed ends the old entry `exited/killed`; an exited entry ignores further events from its process (a session start or another process reopens it); `since_ms` restores the pre-wait and pre-compaction start; ages for GC use the store's `changed_ms`; corrupt files are replaced on write, newer ones left alone; implausible ids refuse the input, implausible paths and tmux names are dropped.
Validation: `run-some.sh --lib live_store` 55 passed; capability and hygiene integration tests passed; `checks.sh` (fmt, clippy -D warnings, rustdoc -D warnings, diff --check) passed; `run-tests.sh` 5032 passed, 1 ignored, 1 failure = the known `discovery::forge` "Text file busy" flake, which passed alone; `tui --snapshot` frames for 9 views byte-identical before and after.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added the live-state store (`src/live_store/`, doc-hidden) of ADR 0204: `$XDG_STATE_HOME/conspectus/live-state.json` with `live-state.lock`, entries keyed `<harness>:<session_id>` holding the `LiveObservation`, cwd, transcript path, tmux socket/session/pane, `instance`/`seq`, `rev`, `changed_ms`, and a private reducer block.

- Event sources: a neutral `LiveEvent` (closed `LiveEventKind`: session started, turn started/ended/failed, retrying, per-agent waits and tool ends, child agents and the background list, compaction start/end, idle confirmed, session ended) folded by one shared reducer; a `Reducer` trait lets the harness stories override single cases.
- Snapshot sources: `LiveSnapshot` replaces an entry by `(instance, seq)`, or by time for a new instance.
- Writers: 200 ms try-lock then `Applied::Dropped`, atomic 0600 writes through the history store's helpers, a store-wide `rev`, supersede of the older entry of a process that moved to a new session id (`exited/ended`, or `exited/killed` when the pid start time changed), GC on write (exited > 1 h, dead pid untouched > 1 day, 512 cap), refusal of implausible ids. Readers: missing, unreadable, corrupt, or newer-schema files read as no live state.
- ADR 0204 refined with these decisions; `docs/library-api.md` lists the module as internal. No CLI, hook, or TUI wiring; no capability entry needed (capability tests pass).

Verified: 55 new unit tests (research-table sequences, snapshot ordering, supersede, GC, tolerant reads, lock timeout, 8 concurrent writer threads, privacy walk and sentinels), `checks.sh` clean, full suite 5032 passed with only the known forge flake (passed alone), `tui --snapshot` frames for 9 views byte-identical.
<!-- SECTION:FINAL_SUMMARY:END -->
