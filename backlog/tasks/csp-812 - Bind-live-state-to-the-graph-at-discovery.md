---
id: CSP-812
title: Bind live state to the graph at discovery
status: Done
assignee:
  - '@claude'
created_date: '2026-11-12 01:47'
updated_date: '2026-11-12 07:19'
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
- [x] #1 After a discovery with a populated store, `node_live` holds one observation per bound entry
- [x] #2 Two agents in two panes of one tmux session each keep their own state
- [x] #3 A pane-only entry shows on the mux through `live_counts_of_mux`, then moves to the session when it appears
- [x] #4 A dead pid yields `exited/killed`; a reused pid (different start time) does too
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Model (src/model/live.rs, graph.rs): `Liveness` gains stable lowercase names, lenient serde, rkyv, and `Default` = `Unknown`; a new `LiveBinding { observation (flattened), liveness }` is the `node_live` value, so the table records the process check the binder made (ADR 0205 decision 4). `live_state_of(id, now_ms)` and `live_counts_of_mux(mux, now_ms)` read that stored liveness instead of taking a closure. `FORMAT_VERSION` 10 -> 11. Update the CSP-808 tests, the live-state fixture, and its golden.
2. Probe (src/live_store/process.rs): `PidProbe` trait (`status(pid) -> PidStatus`, provided `liveness(observation)`), `ProcProbe` over `/proc` (Linux; other platforms answer Unknown); `process_liveness` becomes a thin wrapper so the store GC and the binder share one rule.
3. Binding (src/discovery/live_state.rs, beside hook_sidecar.rs, as attribution of evidence to graph nodes; the store stays graph-agnostic and the TUI already reaches discovery): pure `bind_live(snapshot, file, probe) -> BTreeMap<NodeId, LiveBinding>`: exact (harness, session id) to a live AgentSession; else tmux socket name + session name to a live tmux MuxSession; else unbound; never creates a node; several entries on one node keep the highest rev (ties: newer evidence, then not exited, then key).
4. Mutator `live_state`: provider key + REGISTRY Mutator descriptor + MUTATOR_PROVIDERS; `LocalDiscoveryConfig.live_state_root` (from_env: the store's XDG dir; empty: none; dev scenarios: the world's own state dir; builder methods); runs in apply_mutators right after the hook sidecar, replacing the table each cycle.
5. Tests: unit tests in live_state_tests.rs (exact, pane-only, later session takes over, split panes, dead and reused pid, missing/corrupt store, several entries per node, unbound ignored, gone nodes skipped); discovery tests (fingerprint unchanged by store churn with the mutator, second cycle updates); ReplayWorld integration with a scratch live dir; end-to-end `hook write` then `graph --format json` in tests/hook_write.rs.
6. Docs: ADR 0205 (decisions 1-5 refined), design.md Discovery Strategy bullet, operations.md, library-api.md, CHANGELOG. Verify TUI frames byte-identical (frames.sh), checks.sh, run-tests.sh; finalize.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Decisions: (1) `node_live` values are now `LiveBinding { observation, liveness }`: the binder's pid check is stored (ADR 0205 decision 4), and `live_state_of(id, now)` / `live_counts_of_mux(mux, now)` read it instead of taking a reader closure (the CSP-808 signatures); JSON entries gain `liveness`; `FORMAT_VERSION` 10 -> 11. (2) `bind_live(snapshot, file, probe)` lives in `discovery::live_state` beside `apply_live_state` (binding is attribution of evidence to graph nodes, as `hook_sidecar` does; the store stays graph-agnostic and the TUI already reaches discovery); it takes no clock. (3) `PidProbe`/`PidStatus`/`ProcProbe` in `live_store::process`; the store GC's `process_liveness` now goes through `ProcProbe::system()`. (4) Only live nodes bind (history's gone/unknown skipped); the tmux server is the socket path's last component (`default` = default server), tmux muxes only; two session nodes for one native id bind the lower id; several entries on one node keep the highest rev, then newest observed_ms, then not exited, then lowest key. (5) `LocalDiscoveryConfig.live_state_root`: `from_env` = `LiveStore::from_env` dir (mirrors history: read in every real mode, including no-cache and live snapshot), `empty()` = none, dev scenarios = the world's `state/conspectus`; fixture mode runs no discovery.
Validation: checks.sh passed (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check); run-tests.sh exit 0, 5150 passed, 0 failed, 1 ignored. Each commit's tree was checked (`cargo check --all-targets --all-features` plus its narrow tests). `tui --snapshot` frames of the 9 showcase views were byte-identical between a baseline binary and this change, and so was a live-discovery frame over a scratch home with a populated store (whose `graph --format json` showed the binding). Filed CSP-841 (several pane-only agents in one mux count as one).
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Discovery now binds the live-state store to the graph (ADR 0205). `discovery::live_state::bind_live(snapshot, file, probe)` is pure: an entry goes to the live agent session with its harness and native id, else to the live tmux mux its socket and session name name, else nowhere; it never creates a node, skips history's gone and unknown nodes, and when several entries land on one node keeps the one written last (rev, newest evidence, not exited, key). Each binding stores the pid check of a `PidProbe` (`ProcProbe`: `/proc/<pid>` and stat field 22 on Linux, unknown elsewhere), so `node_live` holds `LiveBinding { observation, liveness }` and `live_state_of(id, now)` / `live_counts_of_mux(mux, now)` need no reader closure; `FORMAT_VERSION` is 11 and graph JSON entries carry `liveness`. The `live_state` mutator (registered beside `hook_sidecar`) runs after the hook sidecar on every discovery and replaces the table from `LocalDiscoveryConfig::live_state_root` (XDG state dir from the environment, none for `empty()`, the world's own dir for dev scenarios); it reads one file, stats one pid per bound entry, starts no process, and runs after the process-tree fingerprint is taken.

Verified: unit tests in `src/discovery/live_state_tests.rs` (exact match, pane-only on the right server, unbound entries, zellij and gone nodes ignored, later session takes over, split panes, dead and reused pids, several entries per node, missing and corrupt stores, the table replaced); discovery tests (populated table after discovery, cold and warm cycles follow the store, fingerprint unchanged by state churn, no root binds nothing); ReplayWorld tests (sidecar-synthesized session, pane-only to session, two agents in one tmux session, vanished pid); an end-to-end `hook write claude-code` then `graph --format json` test; model tests for `Liveness` names and `LiveBinding`. checks.sh passed; run-tests.sh 5150 passed, 0 failed; `tui --snapshot` frames byte-identical for 9 fixture views and a live-discovery view with a populated store. Docs: ADR 0205, design.md, operations.md, library-api.md, CHANGELOG. Follow-up: CSP-841.
<!-- SECTION:FINAL_SUMMARY:END -->
