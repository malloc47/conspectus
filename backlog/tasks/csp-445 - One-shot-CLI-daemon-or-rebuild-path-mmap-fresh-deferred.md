---
id: CSP-445
title: One-shot CLI daemon-or-rebuild path (mmap-fresh deferred)
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 504000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: two new tests in `tests/cli_persist.rs` —
  `table_sessions_dual_writes_graph_bin_alongside_graph_sqlite`
  asserts both files land after a cold rebuild and that
  `graph.bin` validates end-to-end via
  `snapshot::open_mmap` + `deserialize_owned`;
  `table_sessions_with_no_cache_skips_graph_bin_too`
  pins the `--no-cache` half. The pre-existing
  cli_persist matrix (warm start, refresh, corruption
  recovery, backup rotation) still passes —
  `cache_resolved_snapshot`'s SQLite half is unchanged.
  Full suite green: `cargo nextest run --all-targets
  --all-features` at 1778 tests;
  `cargo fmt -- --check` and `cargo clippy --all-targets
  --all-features -- -D warnings` clean.
- Notes: the original mmap-fresh attempt picked the
  slowest class interval (5 min) as the freshness TTL.
  The regression surfaced exactly because mutator
  providers run at sub-second cadence in operator
  workflows; aligning with the heavy-provider TTL was
  the wrong abstraction. A future revival ADR can pick
  between input-mtime tracking, mutator re-run after
  mmap, or daemon-only `graph.bin` writes with an
  invalidation contract.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`warm_start_discover_and_resolve` now opens
with a `try_daemon_snapshot()` call: when
`conspectus serve` is reachable on the socket, the CLI
fetches the resolved snapshot via `client_snapshot()`,
decodes via `snapshot::from_bytes()`, and returns —
skipping discovery + resolve + persist for every CLI
command that uses this helper (`table`, `node show`,
`graph`, `query` until CSP-447). The helper falls
through to the existing CSP-139 phase 4 cold-rebuild
path when the daemon is absent, the snapshot is
unavailable (pre-first-cycle), or the bytes fail to
decode. `--refresh` forces the local path so the
operator-typed "ignore caches, rebuild from disk"
semantic is preserved.
`cache_resolved_snapshot` extends to dual-write
`graph.bin` alongside the legacy `graph.sqlite`: every
successful cold rebuild lands both artifacts so the
next daemon cycle (or a future mmap-fresh consumer
path) can pick the file up immediately. The graph.bin
write is best-effort; failures log a one-line warning
matching the SQLite half. `--no-cache` suppresses both
writes.
**Mmap-fresh branch deferred.** CSP-445's original
scope named (a) daemon, (b) mmap fresh, (c) cold
rebuild. The (b) branch is removed from this iteration:
TOML-rooted mutator providers (`declared`, `aliases`,
`pins` per `cache::MUTATOR_PROVIDERS`) edit files the
wall-clock TTL freshness gate cannot reason about, so a
sub-second TOML edit between two CLI invocations would
let mmap silently shadow a real change. The first
revision shipped the bug; the regression net was
`declared_confirm_in_detailed_graph_preserves_discovered_candidate`
in `tests/cli_smoke.rs`, which failed because a freshly
confirmed declared link did not appear in the
immediately-following `graph` output. A proper mmap-fresh
revival either tracks input mtimes against the artifact
or re-runs the mutator-class providers after the mmap;
both are bigger than the daemonless cold-rebuild cost
(single-digit seconds per ADR 0082) justifies right
now. The artifact write stays so the path is reopenable
later without reshaping the producer side.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-008`
