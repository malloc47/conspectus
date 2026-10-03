---
id: CSP-142
title: Implement `conspectus serve`
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - p7
milestone: m-12
dependencies: []
ordinal: 382000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: 7 integration tests in `tests/cli_serve.rs` —
  populates the cache on first tick, logs startup, shuts down
  cleanly on SIGTERM, ping round-trips, unknown command
  returns a stable error code, refresh advances the cache
  mtime, and `conspectus refresh` exercises both the daemon-
  routed and the in-process-fallback paths.
- Manual checks: `conspectus serve &` followed by
  `conspectus table sessions` in another shell returns the
  warm-cached data; killing the server and rerunning the CLI
  works without intervention; the socket file is unlinked on
  graceful shutdown.
- Deferred follow-ups: rename / declare-link / ignore-link
  over the socket. The daemon does not (yet) hold a long-
  lived writer connection, so one-shot CLI mutations
  continue to serialize against the daemon's writes via
  SQLite's `busy_timeout` — the same coordination peer one-
  shot CLIs already use. Routing those commands through the
  socket is a clean refactor when the writer connection
  becomes dedicated (and would unblock the per-mutation
  audit trail CSP-144 will want).
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed in three layers across the same workstream.
- Layer A: minimal daemon with a single warm-start tick
  loop on the shortest `[server.intervals]` cadence,
  cycle-level failure isolation, stderr logging. Reuses
  `discover_local_warm_with` + `persist_snapshot` so the
  daemon's writes are byte-for-byte equivalent to a one-
  shot `conspectus table` invocation.
- Layer B: per-class scheduler. One thread per ADR 0079
  class (git / mux / harness / forge); each wakes on its
  class's interval and refreshes only its own slice (evict
  + run skipping every other class via the freshness gate).
  A process-local writer Mutex around each cycle keeps two
  class threads from racing on load + persist. Mutex
  poisoning auto-recovers since the on-disk cache is the
  durable state. Adds
  `ProviderClass::{ttl_duration, providers, name, all}` so
  the scheduler can iterate classes generically.
- Layer C: graceful shutdown + mutation socket. ADR 0080
  records the choice of `signal-hook` for SIGINT/SIGTERM
  observability. The shutdown latch is a shared
  `Arc<AtomicBool>` every thread polls between sleeps so
  Ctrl-C is observed within 200ms. The Unix-domain socket
  binds at the canonical path from ADR 0038
  (`$XDG_RUNTIME_DIR/conspectus/server.sock` with TMPDIR
  fallback), mode 0600, length-prefixed JSON framing
  verbatim. v1 dispatch handles `ping` (wire-shape probe)
  and `refresh` (force a cold rebuild on the daemon side);
  `conspectus refresh` is the matching CLI client that
  routes through the socket when present and falls back to
  an in-process cold rebuild when the daemon is absent.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P7-006`
