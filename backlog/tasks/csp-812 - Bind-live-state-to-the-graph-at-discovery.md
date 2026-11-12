---
id: CSP-812
title: Bind live state to the graph at discovery
status: To Do
assignee: []
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 02:27'
labels:
  - live-state
  - ls-bind
milestone: m-26
dependencies:
  - CSP-808
  - CSP-809
documentation:
  - >-
    docs/adr/0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md
priority: high
type: enhancement
ordinal: 898000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
The store is only useful once its entries reach the nodes consumers read. ADR 0205 defines the binding (session by harness and native id, otherwise the mux by tmux socket and session name) and runs it both at discovery and again in the TUI, so a pure function must own it.

Scope: Add the pure `bind_live(snapshot, entries, liveness, now)` and the `live_state` discovery mutator, registered beside the hook sidecar (provider descriptor, eviction, always re-run) so daemon cycles, one-shot cold rebuilds, and the daemonless TUI's background discovery all produce the table. Add a liveness probe that checks an entry's pid and start time against the process table the discovery code already has (Linux `/proc`; other platforms report `Unknown`). Pane-only entries bind to the mux; a later-appearing session takes the state from the mux on the next bind. Unbound entries stay in the store. Live state never creates a node.

TUI changes: none visible. The TUI renders the same frames before and after (check a few views with `conspectus tui --snapshot`).

Tests and checks: Tests with the replay world and a scratch store: exact session match, pane-only to mux, session appearing later, split panes with two agents in one tmux session, dead pid, pid reuse (start time differs), missing store, and that the fingerprint and graph goldens are untouched.

Blockers: The side table and the store.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [ ] #1 After a discovery with a populated store, `node_live` holds one observation per bound entry
- [ ] #2 Two agents in two panes of one tmux session each keep their own state
- [ ] #3 A pane-only entry shows on the mux through `live_counts_of_mux`, then moves to the session when it appears
- [ ] #4 A dead pid yields `exited/killed`; a reused pid (different start time) does too
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [ ] #1 `just check` passes, or `git diff --check` for docs-only changes
- [ ] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->
