---
id: CSP-596
title: Route every subprocess through one module and check shipped source
status: Done
assignee:
  - '@claude'
created_date: '2026-10-13 03:24'
updated_date: '2026-10-13 03:27'
labels:
  - h-caps
milestone: m-11
dependencies: []
priority: high
ordinal: 645000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Scope (ADR 0117): add `src/exec.rs` with `exec::command(Program, binary)` and move all 18 `Command::new` call sites onto it. Add `tests/capabilities_static.rs`, which fails on:
- `Command::new` outside `src/exec.rs`;
- `CommandExt`, `std::net`, and TCP/UDP types;
- `libc` fork/exec/socket/connect/ptrace;
- Unix sockets outside the daemon;
- `libc::kill` outside the teardown.

Test code is exempt.

Tests: the source check itself, plus a planted `Command::new` that it must report.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Shipped code starts processes only through exec::command
- [x] #2 The source check fails on a planted Command::new, network API, or stray signal
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Landed in PR #1. `src/exec.rs` builds every subprocess, and all 18 shipped `Command::new` sites name a `Program`. `tests/capabilities_static.rs` rejects process, network, and signal APIs outside their allowed files. Verified by planting a `Command::new("curl")` in `src/discovery/git.rs`, which was reported with its file and line. Passes `just check` and CI.
<!-- SECTION:FINAL_SUMMARY:END -->
