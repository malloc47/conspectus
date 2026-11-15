---
id: CSP-770
title: Stop fake-binary tests from racing on a busy executable
status: Done
assignee:
  - '@claude'
created_date: '2026-10-28 05:04'
updated_date: '2026-11-15 02:52'
labels:
  - testing
dependencies: []
priority: low
type: bug
ordinal: 856000
---

## Description

<!-- SECTION:DESCRIPTION:BEGIN -->
Tests that write a fake executable (a fake gh in the forge tests, a fake tmux in rebuild::script_tests) and then run it intermittently fail with 'Text file busy' (ETXTBSY). Three separate agents hit it while running the full suite during the TUI framework milestone: once in a forge test, and twice in rebuild::script_tests::the_tmux_commands_a_plan_shows_are_the_arguments_the_runner_executes. The test passes alone and on rerun. The usual cause is a parallel test forking while another test still holds the freshly written file open for writing, so the exec sees it busy. It is unrelated to the TUI work but costs a rerun on every full-suite run.
<!-- SECTION:DESCRIPTION:END -->

## Acceptance Criteria
<!-- AC:BEGIN -->
- [x] #1 Run 200 times as threads of one cargo test process (16 test threads, eight busy loops loading the CPU), the library tests that write and run programs fail in no run, where they failed in 29 runs before the fix; a full cargo test --workspace --all-targets --all-features run passes
- [x] #2 Every program a test runs is written and closed by a short-lived child process before any test can exec it, through one shared helper (tests/support/program.rs), and a scan fails on test code that makes a file executable any other way
<!-- AC:END -->

## Definition of Done
<!-- DOD:BEGIN -->
- [x] #1 `just check` passes, or `git diff --check` for docs-only changes
- [x] #2 Docs and ADRs are updated when behavior, architecture, or workflow change
<!-- DOD:END -->

## Implementation Plan

<!-- SECTION:PLAN:BEGIN -->
1. Reproduce under the threaded runner: run the library test binary (forge, rebuild script, live poller, hook program tests) 200 times with 16 test threads under CPU load, count failing runs, and confirm the cause in the failure text (`Text file busy (os error 26)`).
2. Add one shared helper, `tests/support/program.rs` (`write_program`, `copy_program`), included by the library's unit tests and by integration tests: the file is written by a short-lived child process, so no thread of the test process ever holds a write descriptor a sibling test's fork could inherit; it is executable and closed everywhere when the call returns.
3. Move every test that writes a program (unit and integration) to the helper; give the forge test's second shim its own path; drop the live poller's start-up retry, which only papered over this race.
4. Enforce the rule with a source scan (a mode literal with an execute bit in test code outside the helper fails), rewriting directory permission sites to restore captured permissions instead of literal modes.
5. Record the convention in an ADR and the source-tree rule table; rerun the same stress loop after the fix and expect zero failures; run checks, `just check`, and the full `cargo test` suite.
<!-- SECTION:PLAN:END -->

## Implementation Notes

<!-- SECTION:NOTES:BEGIN -->
Reproduction (before the fix, at 98373421): the library test binary run directly with the tests that write programs (`discovery::forge rebuild::script_tests discovery::live_poller cli::hook_program`, `--test-threads=16`), 200 times while eight busy loops loaded the 8-core machine, failed in 29 runs (31 test failures: 7 `a_cancelled_hosts_lookup_returns_promptly_with_no_answer`, 8 `a_cancellable_gh_call_reads_a_listing_larger_than_a_pipe_buffer`, 8 `a_cancellable_gh_call_classifies_failures_like_a_plain_one`, 4 `a_cancelled_gh_call_kills_its_child_promptly`, 4 `rebuild::script_tests::the_tmux_commands_a_plan_shows_are_the_arguments_the_runner_executes`). 24 failure messages name the cause: `failed to spawn gh binary at /tmp/.../gh ... Text file busy (os error 26)`; the rest are the hosts lookup's `Some([])`, where `authenticated_hosts_until` maps the failed start to an empty list. Even unloaded, 5 of 20 runs failed. The live poller's tests did not fail in the loop; they retried a failed start up to five times, a workaround for this race. Loop script: /tmp/csp-fu/770/stress.sh (logs before.log, after.log).

Approach: write each program from a short-lived `/bin/sh` child (`printf '%s' "$2" > "$1"`, a builtin, so no PATH needed), then chmod by path, so no thread of the test process ever holds a write descriptor a sibling's fork can inherit. A retry in the test helper was not possible (production code starts the program and swallows the error), and rename-into-place does not help (the inherited descriptor names the same inode). Recorded as ADR 0241.

Acceptance criteria made concrete: the brief's targeted loop replaces the 20 full-suite runs. It reproduces the race in about one run in seven (29 of 200), so 200 clean runs are much stronger evidence than 20 full-suite runs, in which the race showed only now and then. "Synced" was dropped: syncing has no bearing on ETXTBSY; closing every write descriptor does.

Validation: after the fix the same loop (stress.sh over the rebuilt library test binary) failed in 0 of 200 runs. `tests/program_hygiene.rs` checks the helper directly (100 programs written and started while four threads fork); the same check with `fs::write` failed in 5 of 5 runs with os error 26. Full `cargo test --workspace --all-targets --all-features` (run-tests.sh): 5,811 passed, 0 failed. `checks.sh` (fmt, clippy -D warnings, rustdoc -D warnings, git diff --check) passed; `just check` passed.
<!-- SECTION:NOTES:END -->

## Final Summary

<!-- SECTION:FINAL_SUMMARY:BEGIN -->
Tests that write a stand-in program and run it no longer fail with "Text file busy" under `cargo test`. The cause, confirmed in the failure text (`failed to spawn gh binary ... Text file busy (os error 26)`): a test wrote its program with `fs::write` in the test process, a sibling test thread forked, and the child held the write descriptor until its exec, so the kernel refused to run the program.

Landed:
- `tests/support/program.rs`: `write_program` writes the file from a short-lived `/bin/sh` child (builtin `printf`) and `copy_program` copies with `cp`, then both chmod 0755 by path, so no thread of the test process ever holds a write descriptor. The library includes it as `crate::test_program`; integration tests include it by path.
- Every test that wrote a program uses it (forge, rebuild script, live poller, hook program unit tests; capabilities, claude_agents_live, cli_contains, cli_deleted_cwd, cli_pin, cli_smoke, hook_write, tui_tmux_e2e). The forge hosts test writes its second stand-in beside the first. The live poller's start-up retry, which only hid this race, is gone.
- `tests/program_hygiene.rs` fails on a mode literal with an execute bit in test code outside the helper, plants a violation in each scanned tree, and checks the helper's promise while four threads fork. Directory permission sites restore what they found instead of literal modes.
- ADR 0241 records the decision; the source-tree rule table lists the scan.

Verified: the library tests that write programs, run 200 times with 16 threads under CPU load, failed in 29 runs before and 0 after. The helper check with `fs::write` instead failed 5 of 5 runs. Full `cargo test --workspace --all-targets --all-features` passed (5,811 tests), as did `checks.sh` and `just check`.
<!-- SECTION:FINAL_SUMMARY:END -->
