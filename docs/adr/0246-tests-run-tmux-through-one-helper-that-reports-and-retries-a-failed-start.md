# ADR 0246: Tests Run tmux Through One Helper That Reports And Retries A Failed Start

## Status

Accepted. Follows [ADR 0241](0241-tests-write-the-programs-they-run-from-a-child-process.md)
(the other flake of this kind that tests had) and
[ADR 0224](0224-local-checks-run-the-workspace-tests-once-through-nextest.md)
(the gate that a flake costs a rerun of).

## Context

Four end-to-end test files start private tmux servers (`tui_tmux_e2e`,
`cli_history`, `cli_pin`, `pane_title_live`), each under a `TMUX_TMPDIR` of
its own, and a full run starts hundreds of them beside the processes the
tests spawn. Now and then one `new-session` fails to start its server. The
pre-main gate failed this way three times in a day
(`no_resume_view_creates_no_state_file_but_a_run_without_it_does`, at the
outer `new-session`), each time passing in isolation and on the rerun, and
each rerun costs a full gate.

The harnesses asserted only the exit status, so a failure said nothing of
why. Six copies of the suite run at once, 204 tests at 48 threads, did not
reproduce it, so the cause is not known.

## Decision

1. **One helper, `tests/support/tmux.rs`.** Every tmux call of those four
   files runs through `tmux::run`, which takes the call's `TMUX_TMPDIR` and
   a closure that builds the call.
2. **A failure says what tmux said.** `tmux::report` names the arguments,
   the exit status, tmux's stderr and stdout, the `TMUX_TMPDIR`, and the
   sockets under it, so a server that never came up can be told from one
   that went away. The e2e harness's asserting calls (`outer_ok`,
   `inner_ok`) and the other three files' `tmux()` print it on failure.
3. **A start that fails the way a loaded machine makes it fail runs again,
   at most three times.** Only a call that can start a server
   (`new-session`, `start-server`) is run again, and only when it failed
   with `server exited unexpectedly`, `lost server`, `failed to connect to
   server`, `no server running`, `create window failed`, or `fork failed`.
   Any other failure, including a refusal such as `duplicate session` or
   `can't find session`, and any call that starts no server, runs once. Each
   retry prints its report to the test's stderr first, so a flake that
   retries is still visible in the output of a failed run.
4. **The helper has tests of its own** (`tests/tmux_support.rs`), against a
   stand-in program that fails as asked and counts its runs.

## Consequences

- A start failure that passes on a second try no longer fails the gate or
  costs a rerun of it.
- When one fails all three times, or fails some other way, the message
  carries tmux's own words, which is what names the cause.
- A retry can mask a real start-up bug that only fails sometimes. The
  signatures retried are tmux's start-up failures, not the TUI's, and each
  retry is printed.
- The cause stays unknown until a failure's report names it; this ADR
  should then be amended, and the retry dropped if the cause is something a
  retry should not hide.

## Alternatives Considered

- **Rerun the failed test in the runner** (nextest retries). It hides every
  flake, this one and the next, and costs the rerun of a whole test.
- **Serialize the tmux tests.** It trades the flake for minutes of run
  time, the cost this was meant to save.
- **Retry any failure of a `new-session`.** A refusal (`duplicate
  session`) is the test's mistake and should fail at once.
