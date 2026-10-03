---
id: CSP-441
title: Implement the snapshot format module
status: Done
assignee: []
created_date: '2026-06-22 21:57'
labels:
  - p11
milestone: m-17
dependencies: []
ordinal: 500000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: nine unit tests in `src/snapshot::tests` —
  `header_round_trips_through_bytes` (byte layout pin),
  `header_rejects_short_buffer`,
  `header_rejects_wrong_magic`,
  `header_rejects_future_format_version`,
  `header_rejects_past_format_version`,
  `write_atomic_then_open_mmap_round_trips` (the
  end-to-end check including `deserialize_owned` equality),
  `open_mmap_rejects_corrupted_payload_byte` (zeroes the
  trailing 64 bytes of the payload — where rkyv lays out
  the root struct + relative pointers — and asserts the
  bytecheck validation pass returns `SnapshotError::Validate`;
  mid-payload byte flips were too lenient because they
  landed in String bodies that bytecheck doesn't
  structurally validate),
  `write_atomic_failure_mid_write_leaves_target_untouched`
  (writes a partial tmp file without renaming, confirms
  the target file is byte-identical to its pre-write
  state),
  `concurrent_reader_holds_old_snapshot_across_writer_rename`
  (validates the POSIX inode-liveness contract from
  ADR 0083 §"Atomicity": a reader's mmap keeps seeing the
  pre-rename payload while a peer writer atomic-renames a
  different snapshot over the same path; a fresh open
  picks up the new content). All nine pass; full suite
  `cargo nextest run --all-targets --all-features` green at
  1770 tests; `cargo fmt -- --check` and `cargo clippy
  --all-targets --all-features -- -D warnings` clean.
- Notes: rkyv 0.8's bytecheck validates structural soundness
  (pointers, lengths, enum discriminants) but not content
  semantics, so a single-byte flip in the middle of a
  String body does not trigger validation failure. The
  "corrupt the trailing edge" strategy is the regression
  net we actually want — it exercises the
  pointers-and-lengths bytecheck cares about. Daemon-side
  stale-tmp-file cleanup (per ADR 0083 §"Atomicity") is a
  CSP-442 concern.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`src/snapshot.rs` (single-file module rather than
a directory; the surface is small enough to not warrant
one) exposes the format primitives:
- `MAGIC: [u8; 8] = *b"CONSPECT"`, `HEADER_LEN = 32`,
  `FORMAT_VERSION: u32 = 1`, `RESERVED_LEN = 16`,
  `MAX_PAYLOAD_LEN = 1 GiB` (the read-side cap to bound
  future pre-allocators).
- `Header { magic, format_version, payload_len, reserved }`
  with `Header::new(payload_len)`, `Header::to_bytes()`,
  and `Header::parse(&[u8])`. Parsing returns typed errors
  so callers distinguish wrong-magic, version-mismatch,
  and truncated headers up front.
- `pub fn write_atomic(path, snapshot)` — `rkyv::to_bytes`
  → write header+payload to `<filename>.tmp.<pid>` →
  `sync_all` → POSIX rename over target → best-effort
  parent-dir `sync_all`. The pid-suffixed tmp path lets
  two concurrent invocations write without clobbering
  each other; the rename arbitrates the final state.
- `SnapshotMmap` (Debug-derived) holds the mmap + parsed
  header. `archived() -> &ArchivedGraphSnapshot` returns a
  zero-cost borrow into the mapped pages via
  `rkyv::access_unchecked` (validation already ran).
  `payload()` exposes the raw archive bytes for callers
  that want them; `header()` returns the parsed header.
- `pub fn open_mmap(path)` — opens, mmaps, parses the
  header, validates the payload via
  `rkyv::access::<ArchivedGraphSnapshot, rancor::Error>`
  (the bytecheck pass).
- `pub fn open_mmap_unvalidated(path)` — same minus the
  validation pass, for callers that trust the source
  (e.g. the CSP-443 socket-served bytes path).
- `pub fn deserialize_owned(&SnapshotMmap) ->
  Result<GraphSnapshot>` — escape hatch for tests /
  JSON-dump call sites; rkyv-deserializes the archive
  into an owned tree.
- `SnapshotError` enum (thiserror-derived) discriminates
  `Io`, `Truncated`, `WrongMagic`,
  `IncompatibleVersion { expected, found }`,
  `PayloadTooLarge`, and the three rkyv stages
  (`Serialize`, `Validate`, `Deserialize`).
`Cargo.toml` gains `memmap2 = "0.9"` (~2k Rust lines, no
transitives). `src/lib.rs` registers `pub mod snapshot;`.
The module has zero callers in this story — it is the
library piece CSP-442 (daemon writes), CSP-443 (socket
`snapshot` command), CSP-444 (TUI cutover), and CSP-445
(CLI mmap-or-rebuild) all consume.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P11-004`
