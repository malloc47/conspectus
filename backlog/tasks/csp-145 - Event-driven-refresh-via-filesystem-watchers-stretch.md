---
id: CSP-145
title: Event-driven refresh via filesystem watchers (stretch)
status: Done
assignee: []
created_date: '2026-05-18 14:50'
labels:
  - p7
milestone: m-12
dependencies: []
ordinal: 385000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Tests: 3 watcher unit tests (queue semantics, empty-path
  no-op, notify on real file create) + 1 end-to-end
  integration test (`serve_harness_watcher_fires_on_state_dir_change`)
  that spawns the daemon, writes a file in the harness
  state dir, and asserts a harness cycle starts within 3
  seconds — comfortably under the 5-second polling fallback
  so the assertion can only pass when the watcher actually
  wakes the scheduler.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Harness state directories now drive event-driven
refresh via the `notify` crate per ADR 0081.
- `src/server/watcher.rs` owns the Watcher trait
  ({Changed, Timeout, ShuttingDown} events), a notify-
  backed implementation, a NullWatcher fallback, and a
  FakeWatcher test-support type.
- The harness class scheduler now waits on either a
  filesystem change OR its 5-second interval, whichever
  fires first. A new session appearing in a harness state
  dir surfaces in `graph.sqlite` within milliseconds
  rather than the next 5-second poll.
- Failure to install (rlimit, EACCES, unsupported fs)
  falls back to interval polling with a one-line stderr
  warning. The polling cadence is unchanged, so the
  degradation is a latency loss, never a correctness loss.
- The other classes (git / mux / forge) keep
  NullWatcher; git-refs and `.conspectus.toml` watchers
  are natural extensions that land separately when the
  operator-pain or implementation-readiness signal
  arrives.
- Promoted `src/server.rs` → `src/server/mod.rs` so the
  watcher submodule has a sensible home.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `P7-009`
