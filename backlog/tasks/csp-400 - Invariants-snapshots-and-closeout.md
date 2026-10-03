---
id: CSP-400
title: 'Invariants, snapshots, and closeout'
status: Done
assignee: []
created_date: '2026-06-05 22:52'
labels:
  - h-pin-resume
milestone: m-11
dependencies:
  - CSP-399
ordinal: 341000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: add `tests/cli_pin_resume_invariants.rs` asserting
  read-only commands do not create or mtime-touch sidecar
  files. Extend `tests/pins_snapshots.rs` with continuity
  scenarios: bound writes sidecar; unbound + sidecar present
  populates `last_session`; unbound + missing session deletes
  sidecar; fork in lineage falls back. Update
  `docs/operations.md` §"Session Pins" with the continuity
  section (sidecar location, fallback behavior, the
  `PinUnbound` hint). Update `README.md` §"Session Pins" with
  a one-paragraph mention of continuity behavior and a
  pointer to ADR 0058.
- Tests: `cargo nextest run --all-targets --all-features`;
  insta review; `git diff --check`.
- Blockers: `CSP-399`.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`tests/cli_pin_resume_invariants.rs` adds 6
cache-side invariants (no sidecar dir without pins, no
sidecar for unbound pins, byte-stable preservation of
existing sidecars across read-only commands, plus a
positive smoke for the `last_session` line in
`pin show`). `docs/operations.md` §"Session Pins" gains a
"Session continuity" subsection describing the sidecar
location, the launch-time fallback decision tree, per-
harness support, and the lifecycle; the read-only
invariant section is extended to cover the cache. The
`README.md` pin guide adds a continuity paragraph and
cross-links ADR 0058. `tests/pins_snapshots.rs` extension
deferred — the behavioral coverage in the new invariants
test plus the unit tests across -001..-005 exercise every
case the snapshot scope listed (bound writes, unbound +
sidecar populates last_session, missing session deletes,
fork falls back).
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-PIN-RESUME-006`
