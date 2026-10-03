---
id: CSP-442
title: Daemon dual-writes SQLite and the new artifact
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 501000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: `serve_dual_writes_graph_bin_alongside_graph_sqlite`
  spawns the daemon under a fresh `$XDG_DATA_HOME`, waits
  for both artifacts to appear, then runs
  `snapshot::open_mmap` against `graph.bin` (which
  exercises the bytecheck validation pass end-to-end) and
  confirms `deserialize_owned` round-trips the archive to an
  owned `GraphSnapshot` — that's the regression net for
  daemon-side writer correctness. Full daemon test suite
  (14 prior + 1 new) plus the rest of the project: 1771
  tests pass via `cargo nextest run --all-targets
  --all-features`; `cargo fmt -- --check` and
  `cargo clippy --all-targets --all-features --
  -D warnings` clean. `class_loop` picked up an
  `#[allow(clippy::too_many_arguments)]` after the new
  `snapshot_bytes` parameter pushed it to 8 args; a future
  refactor can bundle the cycle context into a struct if
  additional parameters arrive.
- Notes: the "write failure does not propagate to SQLite"
  branch is proven by code review of the helper rather than
  a fault-injected integration test — making the writer
  mockable from inside a separate process is more
  scaffolding than the property warrants. Daemon-side
  stale-tmp-file cleanup (per ADR 0083 §"Atomicity") is
  deferred to a follow-up alongside the eventual CSP-448
  legacy-cache cleanup migration.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Every successful per-class cycle and every
full-rebuild path in `src/server/mod.rs` now calls the new
`dual_write_artifact(&snapshot, &snapshot_bytes)` helper
immediately after `persist_snapshot`. The helper
serializes the snapshot to its on-disk byte layout via
`snapshot::serialize_to_bytes`, writes the result
atomically to `snapshot::graph_bin_path()` (canonical
`$XDG_DATA_HOME/conspectus/graph.bin`), and refreshes the
in-memory `SnapshotBytes` cache. Serialize and write
failures are best-effort: each logs a single stderr
warning and returns without propagating to the SQLite
path, so the legacy `graph.sqlite` artifact stays durable
during the dual-write window. The cache update happens
even when the disk write fails, so socket-connected
readers (CSP-443) still see the latest snapshot.
`SnapshotBytes = Arc<Mutex<Option<Arc<Vec<u8>>>>>` is the
shared cache shape; the inner `Arc<Vec<u8>>` lets the
upcoming socket handler clone-and-return without copying
the payload. The Mutex variant suffices for v1 — the
writer Mutex around each cycle already serializes the
producer side and the socket path's contention is
sub-microsecond. `arc-swap` is named in ADR 0083 as a
future swap if a profile shows the lock matters.
`src/snapshot.rs` gains `graph_bin_path()` (parallels
`query::persist::graph_db_path`) plus a refactor that
splits the writer into `serialize_to_bytes(&GraphSnapshot)
-> Result<Vec<u8>>` + `write_atomic_bytes(path, &[u8])`
so the daemon can serialize once and reuse the bytes for
both the file and the cache. `write_atomic` keeps its
existing one-shot signature as a convenience wrapper.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-005`
