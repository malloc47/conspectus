---
id: CSP-402
title: 'Record the harness pid, not the hook writer''s pid, in hook sidecar records'
status: Done
assignee: []
created_date: '2026-06-08 02:02'
labels:
  - h-muxproc
milestone: m-11
dependencies: []
ordinal: 268000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Problem: `conspectus hook write claude-code` (and the codex /
  opencode variants) persist `record.pid = std::process::id()` in
  `src/cli.rs:417`, but `std::process::id()` is the pid of the
  transient `conspectus hook write …` child process, not of the
  long-lived claude / codex / opencode process that fired the
  hook. That child exits within milliseconds, so by the time any
  later discovery pass runs `process_is_live(record.pid)` in
  `src/discovery/hook_sidecar.rs:401`, the recorded pid is
  guaranteed to be dead. The hook adapter then marks every record
  `LinkState::Ignored` with reason "hook record pid N is no longer
  active". Live snapshot evidence (the 2026-06-07
  `agentdeck_-local-command-caveat-…-573ac208` mux case): 42 of 42
  claude-code `hook_sidecar` candidates resolved to `state:
  ignored`, hook evidence contributed zero usable input to the
  resolver, and the mux fell back to the stale launch-argv
  candidate, attributing the mux to a 14-day-stale resume parent
  instead of the live session. The earlier "hook records solve
  `CSP-227`" claim in that story is contradicted in practice
  by this writer-pid bug — every hook record is born stillborn.
- Scope: in the `conspectus hook write <harness>` writers (claude,
  codex, opencode in `src/cli.rs`), resolve the harness pid before
  handing it to the `*_record_from_payload` builders. Walk up
  `/proc/<self>/stat`'s `ppid` chain past `sh` / `bash` / wrapper
  layers until a process whose `comm` matches the expected harness
  binary set (`claude`, `claude-code`, `codex`, `opencode`, plus any
  aliases) is found, and record that pid as `record.pid`. Keep
  `record.ppid = parent_pid()` (the immediate parent) for diagnostic
  use. On Linux, walking `/proc/<pid>/stat` is enough; non-Linux can
  keep the existing best-effort behavior (`pid = 0` falls through
  the liveness check at `process_is_live`'s `<= 0` guard, leaving
  the record active rather than stillborn). If a hook payload field
  carries the harness pid directly (claude's `pid` field, codex's
  process metadata), prefer that over the proc walk to avoid races
  in deeply-wrapped invocations.
- Tests: payload-builder unit tests asserting the recorded `pid` is
  the harness pid, not the writer pid, given a mocked process tree
  (writer → sh → harness → ...). Round-trip integration test
  asserting a written record survives `process_is_live` for the
  real harness pid lifetime. Regression test in
  `src/discovery/hook_sidecar.rs` asserting that with the
  harness-pid convention, fresh hook records produce Active
  `LinkedToMux` candidates instead of `Ignored`. Use the existing
  `testing_replay` fixtures or extend them to capture the
  writer→shell→harness chain.
- Manual checks: run `conspectus hook init claude-code`, start a
  live claude session, capture the SQLite row, and confirm the
  recorded `pid` matches `pgrep -x claude` rather than a long-dead
  `conspectus` pid. Run `conspectus graph --format json` and
  confirm at least one `hook_sidecar` candidate for the live
  session has `state: active`.
- Related: `CSP-226`, `CSP-227`, `CSP-249`,
  `CSP-403`, ADR 0028.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Outcome (2026-09-30, `CSP-538`): landed in `4c63044`.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `H-MUXPROC-020`
