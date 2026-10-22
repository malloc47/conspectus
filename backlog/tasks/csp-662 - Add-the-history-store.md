---
id: CSP-662
title: Add the history store
status: To Do
assignee: []
created_date: '2026-10-22 01:10'
labels:
  - history-pins
milestone: m-23
dependencies:
  - CSP-656
documentation:
  - docs/plans/history-rebuild-and-pin-slots.md
priority: high
type: feature
ordinal: 720000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
History is the layer that lets any observed mux, worktree, or repo be rebuilt without having been pinned. It has to survive snapshot format bumps (so it can't live in `graph.bin`), and the daemon, a daemonless TUI, and one-shot CLI runs may all write it at once.

- Scope: a module owning `$XDG_STATE_HOME/conspectus/history/` (per-kind JSON files with `schema_version`, one `history.lock`); stable random record ids; an exclusive advisory lock via `std::fs::File::lock`, re-read, merge, `write_atomic`; a commutative merge (min `first_seen`, max `last_seen`, union then truncate hosted sessions, create missing); a 10-minute rewrite cadence for unchanged live records; unknown fields round-trip; no writes under `--no-cache`, fixture and snapshot modes, the dev scenario builder, or without a state home; fixtures can supply history.
- Tests: unit tests for merge order independence, a stale writer not undoing a reap, cadence, and unknown-field round-trip; a test with several processes writing concurrently.
- Manual checks: run the TUI and a CLI command without a daemon at the same time and confirm the files stay valid.
- Blockers: the history ADR.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 Records persist per kind under `$XDG_STATE_HOME/conspectus/history/` with stable ids and a schema version, and unknown fields survive a rewrite
- [ ] #2 Concurrent writers never lose each other's observations, and a reaped record comes back only when a writer freshly observes the entity
- [ ] #3 Read-only modes and fixture runs never create or modify the history directory
- [ ] #4 No new crate dependency is added
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
