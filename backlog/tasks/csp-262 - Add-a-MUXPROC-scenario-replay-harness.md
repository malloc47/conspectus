---
id: CSP-262
title: Add a MUXPROC scenario replay harness
status: Done
assignee: []
created_date: '2026-05-24 20:27'
labels:
  - test
milestone: m-11
dependencies: []
ordinal: 273000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
- Scope: introduce a test support layer that can build a complete
  synthetic local world from small scenario inputs: harness state
  roots, hook SQLite records, fake tmux rows with active-pane
  command/pid/cwd, and injected active-pane fd evidence. Run the
  same pipeline an operator relies on: discovery, resolution, and
  sessions row-tree projection. Keep it deterministic and free of
  real tmux, real `/proc`, real home directories, or network
  access.
- Tests: self-tests for the harness itself covering empty worlds,
  one harness session plus one mux, hook SQLite record insertion,
  fake fd evidence injection, and path normalization for stable
  snapshots.
- Manual checks: none; this is infrastructure for automated
  regression replay.
- Blockers: none.
<!-- SECTION:DESCRIPTION:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Added `tests/support/replay.rs`, a deterministic
integration-test harness that builds synthetic local worlds from
temp harness state roots, fake tmux rows with active-pane process
fields, hook sidecar SQLite records, and injected active-pane fd
target paths. Replay runs the operator pipeline through local
discovery, fd-evidence inference, resolution, and the sessions
row-tree projection without real tmux, real `/proc`, real home
directories, or network access. Added five self-tests in
`tests/testing_replay.rs` covering empty worlds, one harness
session plus one mux, hook SQLite record insertion, fake fd
evidence injection, and temp-path normalization.
<!-- SECTION:FINAL_SUMMARY:END -->

Legacy ID: `TEST-001`
