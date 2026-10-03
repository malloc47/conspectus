---
id: CSP-505
title: Debug `conspectus serve` resource usage
status: Done
assignee: []
created_date: '2026-07-28 03:30'
labels:
  - h-serve-perf
milestone: m-18
dependencies: []
ordinal: 530000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: the `serve` daemon (ADR 0038 / ADR 0079,
  `src/server/mod.rs`) consumes a nontrivial amount of CPU/memory at
  idle. Profile a running instance to attribute cost across the
  per-class refresh loops (harness=5s, mux=5s, git=30s, forge=5m),
  subprocess spawns (`tmux`, `git`, `gh`), snapshot allocation, and
  any busy-wait in the event loop. Produce a findings note, then land
  the low-risk wins (e.g. coalescing spawns, widening default
  intervals, avoiding full re-scans when inputs are unchanged) behind
  the existing config knobs.
- Tests: whatever the diagnosis warrants — a spawn-count assertion, an
  idle-tick allocation check, or interval-driven scheduling tests.
- Blockers: diagnosis first; fix direction depends on findings.
- Diagnosis (done): recorded in ADR 0091. Dominant idle cost is the
  process-tree `/proc` walk (`cross_link::active_harness_pids_per_mux`)
  which `apply_mutators` runs on *every* discovery cycle, so it fires
  ~every 2.5s (harness+mux staggered). Secondary: full re-resolve +
  `graph.bin` re-serialize/write every cycle (a byte-level publish
  skip does NOT help — `node_provenance.freshness_epoch` churns each
  cycle). Minor: `LocalDiscoveryConfig::from_env()` rebuilt per tick;
  200ms shutdown-poll floor (negligible).
- Decision: chose the deep fix (class-gated mutators, ADR 0091).
  Implementation (landed):
  - **CSP-505.01** Class-gate the process-tree mutator: `discover_local_warm_with` defers eviction of the `PROCESS_TREE_MUTATORS` slice…
  - **CSP-505.02** Dropped: `LocalDiscoveryConfig` is consumed per call (drains boxed backends) and `from_env()` is env-reads only…
  - **CSP-505.03** Cut the per-cycle resolve/publish cost (deferred; revisit only on the triggers below)
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SERVE-PERF-001`
