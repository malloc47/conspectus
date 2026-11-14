---
id: CSP-872
title: Bound the daemon socket connect when the listener's backlog is full
status: Done
assignee:
  - '@claude'
created_date: '2026-11-13 19:46'
updated_date: '2026-11-14 01:58'
labels:
  - live-state
  - ls-fix
milestone: m-26
dependencies: []
priority: low
ordinal: 952000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Every daemon socket request now times out on its reads and writes (ADR 0219), but `UnixStream::connect` itself has no bound. A daemon whose accept loop has stopped (the process stopped with SIGSTOP, say) still lets connections queue in the listener's backlog, so calls time out on their first read; once enough abandoned connections fill the backlog, `connect` blocks and the bound never starts. Surfaced while bounding the socket clients (CSP-836).
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A client call against a listener whose backlog is full ends within its request's bound
- [x] #2 A test fills a listener's backlog and shows the call returns
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. In src/server/mod.rs, make the client's socket with libc, give it the request's bound as its send and receive timeouts (std's set_write_timeout / set_read_timeout on the UnixStream it becomes), and only then connect. Linux bounds a stream connect that waits for room in a full listener backlog by the socket's send timeout and then fails with EAGAIN, which the client reports as TimedOut; elsewhere a full backlog refuses at once (NoDaemon, unchanged). No dependency: libc is already one. The per-request bounds of ADR 0219 are unchanged.
2. Allow `libc::socket` and `libc::connect` in src/server/mod.rs only in tests/capabilities_static.rs (the manifest's AF_UNIX connect to the daemon socket is unchanged; the traced test sees the same syscalls).
3. Test in server_tests.rs: a listener whose backlog is set to 0 and filled by one queued connection; the call returns TimedOut within its bound (run on a helper thread with a deadline so a regression fails instead of hanging). Existing tests keep covering no daemon, a silent daemon, a hang-up, and a prompt answer.
4. Amend ADR 0219 (its consequence said connect is unbounded), CHANGELOG, operations.md where the bounds are listed.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Verified the kernel behavior on this host (Linux 7.0): with the listener's backlog set to 0 and one connection queued, the bounded connect returns TimedOut after 200 ms (three runs, 0.20-0.21 s each); with the old `UnixStream::connect` the same test did not return within 10 s. The source-scan rules for `libc::socket` and `libc::connect` now allow src/server/mod.rs only; the traced capability test passes unchanged (same syscalls, same path). A helper-thread connect was rejected: it leaves one blocked thread and descriptor per call while the daemon stays stopped, which a TUI's refresh worker would accumulate.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
`call_socket` (src/server/mod.rs) now connects through `connect_within`: the socket is made with libc (SOCK_CLOEXEC on Linux, FD_CLOEXEC elsewhere), given the request's bound as its send and receive timeouts, and connected after. Linux bounds a connect waiting for room in a full backlog by the send timeout and fails with EAGAIN, reported as `TimedOut`, so each caller falls back as ADR 0219 decision 3 says; elsewhere a full backlog refuses at once (NoDaemon, unchanged). ADR 0219's per-request bounds are unchanged and now cover the connect. No dependency added; tests/capabilities_static.rs allows `libc::socket` and `libc::connect` in src/server/mod.rs only; docs/capabilities.toml unchanged.

Test: `a_daemon_whose_backlog_is_full_times_the_call_out` sets a listener's backlog to 0, queues one connection, and asserts the call returns TimedOut within its 200 ms bound (it ran on a helper thread with a 10 s deadline, which the old connect missed). ADR 0219 amended, operations.md and CHANGELOG updated. `checks.sh` and `just check` (5704 passed, traced capability test included) green.
<!-- SECTION:FINAL_SUMMARY:END -->
