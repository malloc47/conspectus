---
id: CSP-809
title: Add the live-state store
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 02:25'
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
- [ ] #1 The store reads and writes `live-state.json` atomically under a lock and never blocks a writer more than 200 ms
- [ ] #2 Entries are keyed by harness and session id and carry the tmux socket, session, and pane when known
- [ ] #3 A new session id from the same pid marks the old entry `exited/ended`
- [ ] #4 A reader treats any missing, corrupt, or newer-schema file as no live state
- [ ] #5 No payload text can reach the file; a sentinel test enforces it
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
