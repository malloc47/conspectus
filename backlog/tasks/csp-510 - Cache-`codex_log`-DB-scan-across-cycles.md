---
id: CSP-510
title: Cache `codex_log` DB scan across cycles
status: Done
assignee: []
created_date: '2026-07-28 15:27'
labels:
  - h-serve-perf
milestone: m-18
dependencies: []
ordinal: 534000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: after 001a a min-cycle-gap floor for watcher-driven class
  ticks landed (`server::min_cycle_gap` clamped to `[250ms, 2s]`),
  but the harness class still fired at ~0.8 Hz and each cycle
  re-opened `~/.codex/logs_<N>.sqlite` and ran a
  `LIKE 'pid:<pid>:%'` scan per live Codex pid. The `logs` table
  has no `(process_uuid, ts)` index; on operator boxes with a
  ~20 MB log DB that was ~168 MB/s of cached reads / ~12k syscr/s
  on the harness thread — the dominant post-throttle residual cost.
- Landed: process-local `QUERY_CACHE` in `codex_log.rs` keyed by
  (db path, mtime_ns, size, sorted candidate pid list, cached
  ts_floor). On cache hit the DB is not opened at all; observations
  replay through the same emit path so the resulting snapshot
  mutations are identical to the query path. The `cached_ts_floor`
  field guards against a widened window returning stale "no
  observation" — a lower ts_floor invalidates the cache. Verified
  by three new tests using a `QUERY_COUNT` counter under a serial
  `CACHE_TEST_LOCK` (cache-hit skips SQLite entirely, mtime advance
  forces re-query, expanded pid set forces re-query) plus the 11
  pre-existing codex_log tests including the widening case.
<!-- SECTION:DESCRIPTION:END -->

Legacy ID: `H-SERVE-PERF-002`
