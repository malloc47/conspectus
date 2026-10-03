---
id: CSP-444
title: Cut the TUI refresh over to socket-served snapshots
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 503000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: three new `src/snapshot::tests` cover the new
  helper:
  `from_bytes_round_trips_a_serialized_snapshot` exercises
  the happy path with the sample fixture used elsewhere in
  the suite; `from_bytes_rejects_short_buffer` and
  `from_bytes_rejects_wrong_magic` are the negative-path
  pins. The TUI-side `try_daemon_snapshot` helper isn't
  directly unit-tested because the global socket-path env
  interacts badly with parallel test isolation; the
  existing `serve_socket_snapshot_command_returns_graph_bin_bytes`
  plus the `from_bytes` round-trip together cover the
  daemon→bytes→snapshot chain end-to-end, and the TUI
  helper is a thin combinator over both. Full suite green:
  `cargo nextest run --all-targets --all-features` at 1776
  tests; `cargo fmt -- --check` and `cargo clippy
  --all-targets --all-features -- -D warnings` clean.
- Notes: the "connection held open for the TUI session"
  optimization from the original story scope is deferred —
  the current per-tick connect-and-close is sub-millisecond
  on Unix domain sockets and the TUI's refresh cadence is
  2-30 seconds; the optimization would require extending
  the ADR 0038 framing to multi-request connections (the
  current contract is "one request per connection"), which
  is bigger scope than the per-tick latency justifies. The
  wholesale `read_snapshot` site cleanup is folded into
  CSP-448 alongside the broader SQLite teardown.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Focused cutover at the TUI's refresh entry
point. `src/tui/runtime.rs::discover_and_resolve` now
short-circuits via a new `try_daemon_snapshot()` helper:
when `conspectus serve` is reachable on the socket, the
TUI fetches the already-resolved snapshot via
`client_snapshot()`, decodes it with
`snapshot::from_bytes()`, and returns — skipping the
entire local discovery + resolve + persist cycle. When
the daemon is absent, returns
`snapshot_unavailable`, the transport fails, or the
bytes are malformed, the helper returns `None` and the
pre-cutover local-discovery path runs verbatim. Silent
fallback (no stderr noise) matches the rest of the TUI's
refresh path. `RunConfig::refresh` (operator-forced cold
scan via `--refresh`) bypasses the daemon path so the
flag's semantic is preserved.
The 14 `read_snapshot(db.conn())` consumer sites across
`tui/runtime.rs`, `tui/app.rs`, `tui/actions.rs`,
`tui/detail.rs`, `tui/explorer.rs`,
`tui/rows/sessions.rs`, `tui/rows/mux.rs` are
**deliberately left unchanged for this story**: the
materialized in-memory `GraphDb` is still populated by
the TUI shell as before, and those sites read from it.
Rewriting them to consume the daemon's snapshot directly
is a wholesale read-path refactor — proper scope for
CSP-448 (delete `src/query/`) where the SQLite read
surface goes away entirely. CSP-444's win is the *outer
refresh*: skipping discovery when the daemon already
holds the answer.
`src/snapshot.rs` picks up `from_bytes(&[u8]) ->
Result<GraphSnapshot>` for the in-memory decode path the
TUI uses — no detour through a tmp file when the bytes
are already in memory. Validation policy mirrors
`open_mmap`: bytecheck validates the payload, header
parsing rejects wrong magic / version up front, typed
errors all the way through.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-007`
