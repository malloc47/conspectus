---
id: CSP-886
title: >-
  Make the tmux end-to-end harness report why an outer tmux server failed to
  start
status: Done
assignee:
  - '@claude'
created_date: '2026-11-15 03:14'
updated_date: '2026-11-17 13:55'
labels:
  - test
dependencies: []
priority: low
type: bug
ordinal: 963000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
During the pre-main gate for CSP-770 (commit 7bfe734b), one full `cargo test --workspace --all-targets --all-features` run failed `no_resume_view_creates_no_state_file_but_a_run_without_it_does` at tests/tui_tmux_e2e.rs:238: the outer tmux `new-session` returned a failure status. The same commit passed that binary 5 times in isolation and the whole suite twice under nextest and once more under `cargo test`, so it is a start-up flake under load, the same class as the tmux 'server exited unexpectedly' failures seen in tests/cli_history.rs. The harness asserts only the exit status, so the failure says nothing about why tmux refused (socket path, permissions, server crash, resource limits). Make the harness's tmux helpers include tmux's stderr and the socket path in the assertion message, and decide whether a bounded retry of the server start is warranted once the cause is known.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 A failed tmux command in the end-to-end harnesses reports tmux's stderr and the socket path in the test failure
- [ ] #2 If the cause is identified, the harness handles it (for example a bounded retry of server start), and the change is explained in the test module's docs
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
A failed tmux call in the four end-to-end harnesses (`tui_tmux_e2e`, `cli_history`, `cli_pin`, `pane_title_live`) now reports tmux's stderr and stdout, the arguments, the exit status, the `TMUX_TMPDIR`, and the sockets under it, through one shared helper (`tests/support/tmux.rs`, ADR 0246, tested in `tests/tmux_support.rs` against a stand-in program). A call that starts a server runs again, at most three times, only when tmux said it failed to start (`server exited unexpectedly`, `lost server`, `failed to connect to server`, `no server running`, `create window failed`, `fork failed`); a refusal such as `duplicate session`, and any call that starts no server, runs once, and each retry prints its report. The cause of the gate's occasional `new-session` failure is not identified: six copies of the e2e suite at once (204 tests, 48 threads) and 300 server starts under CPU load did not reproduce it, so the second criterion is left unchecked: the retry is a bounded mitigation, not a diagnosed fix, and ADR 0246 says to revisit it when a failure's report names the cause. Gate: as for CSP-894.
<!-- SECTION:FINAL_SUMMARY:END -->
