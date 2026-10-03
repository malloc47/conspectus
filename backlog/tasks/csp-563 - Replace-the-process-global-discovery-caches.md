---
id: CSP-563
title: Replace the process-global discovery caches
status: Done
assignee: []
created_date: '2026-10-01 00:23'
labels:
  - h-rust
milestone: m-19
dependencies: []
ordinal: 572000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: nine `static` caches hold discovery state for the whole
  process (`PROBE_CACHE` in `discovery/git.rs`, `TMUX_CACHE`,
  `ZELLIJ_CACHE`, `FORGE_CACHE`, `QUERY_CACHE` in `codex_log.rs`,
  `ROLLOUT_SCAN_CACHE`, `SESSION_SCAN_CACHE`,
  `SQLITE_SESSIONS_CACHE`, and `LAST_MUX_HARNESS_FINGERPRINT`). Each
  repeats the same get/store/reset functions and calls
  `lock().unwrap()`, so a poisoned lock panics where the daemon
  recovers. Tests need four serial `*_TEST_LOCK` mutexes, and two
  `pub #[doc(hidden)] reset_*_for_tests` functions leak into the
  public API. Five adapters also hand-compute an `i128` nanosecond
  mtime plus a size as a fingerprint, where comparing the
  `SystemTime` from `metadata.modified()` would do, and the git
  probe cache returns `Option<Option<GitProbeResult>>` to separate a
  miss from a cached negative.
- Plan, two commits: (a) behavior-preserving dedupe into a small
  `TtlCache<T>` and a `FileStamp`-keyed cache, with an explicit enum
  for hit, miss, and cached-negative; (b) move the caches into a
  `DiscoveryCaches` value owned by the caller (daemon state, TUI
  loop, one-shot CLI) and passed through `LocalDiscoveryConfig`,
  then delete the test locks and reset functions.
- ADR: (b) changes `LocalDiscoveryConfig`, which is part of the
  library facade (ADR 0015); amend ADR 0091, which introduced most
  of these caches.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
ADR 0099. `discovery::DiscoveryCaches` holds all nine
caches, built from the `TtlCache`/`StampedMap`/`FileStamp`
helpers in `discovery/memo.rs`. The daemon and TUI own one each and
pass it through `LocalDiscoveryConfig::with_caches`; one-shot
commands start empty. `GitProbe::probe` no longer caches
(`probe_cached` does). The four test locks, the reset functions,
and the global spawn/query counters are gone. The daemon scheduler
now takes its shared context struct instead of seven arguments.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-RUST-010`
