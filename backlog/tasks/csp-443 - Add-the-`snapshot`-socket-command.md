---
id: CSP-443
title: Add the `snapshot` socket command
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 502000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: two integration tests in `tests/cli_serve.rs` —
  `serve_socket_snapshot_command_returns_graph_bin_bytes`
  spawns the daemon, waits for `graph.bin` to land,
  requests a snapshot over the socket via hand-rolled
  framing (proves the wire shape independently of the
  typed client), confirms the decoded bytes equal the
  on-disk artifact byte-for-byte, and re-validates them
  via `snapshot::open_mmap` + `deserialize_owned` to
  assert they form a structurally sound archive.
  `serve_socket_snapshot_command_errors_before_first_cycle`
  races the daemon's first cycle to exercise the
  `snapshot_unavailable` path — accepts either outcome
  (the cycle may finish before the request lands on a
  fast machine) but verifies the error envelope carries
  the documented code when it does fire. Full suite:
  `cargo nextest run --all-targets --all-features` green
  at 1773 tests; `cargo fmt -- --check` and
  `cargo clippy --all-targets --all-features --
  -D warnings` clean.
- Notes: the typed `client_snapshot` helper isn't directly
  unit-tested — its surface is thin (frame → base64 →
  bytes) and the integration test exercises the same wire
  path. CSP-444 (TUI cutover) will be the first real
  consumer.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
The daemon's `dispatch` table picks up a
`snapshot` arm (`handle_snapshot`) that reads the cached
bytes from CSP-442's `SnapshotBytes`, base64-encodes
them, and returns under `data.bytes`. Pre-first-cycle
callers get a `result: "error"` envelope with code
`snapshot_unavailable` and a "daemon has not completed
its first cycle" message so the caller can fall through
rather than block (CSP-445's mmap-or-rebuild path is the
natural consumer). The handler clones the `Arc<Vec<u8>>`
out of the cache under the Mutex (held only long enough
to copy the Arc handle), drops the guard, then encodes —
so a long base64 pass does not block class threads from
updating the cache.
`client_snapshot()` in `src/server/mod.rs` is the typed
helper symmetric with `client_status` / `client_refresh`:
sends the framed request, base64-decodes the response
`data.bytes`, returns `ClientOutcome<Vec<u8>>`. Daemon
errors and transport errors flow through the existing
`ClientOutcome` variants unchanged.
Framing decision: base64 in JSON rather than a
binary-frame variant. Tradeoff documented in the
`Cargo.toml` `base64` comment block — at conspectus's
single-digit-MB snapshot scale the 4/3 expansion is
irrelevant and keeps the protocol uniform. The 16 MiB
`MAX_FRAME_BYTES` cap in `read_frame` accommodates
snapshots up to ~12 MiB raw; if real-world graphs ever
push past that, the binary-frame variant becomes the
upgrade path.
`Cargo.toml` gains `base64 = "0.22"` (~50 KB pure Rust,
MIT/Apache, ubiquitous in the ecosystem). Small enough
to fit the dep policy without an ADR; the rationale is
captured in the Cargo.toml comment block + this story
outcome.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-006`
