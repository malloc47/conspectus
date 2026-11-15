# ADR 0241: Tests Write The Programs They Run From A Child Process

## Status

Accepted. Settles the `Text file busy` failure that
[ADR 0224](0224-local-checks-run-the-workspace-tests-once-through-nextest.md)
left for CI's `cargo test` step to catch.

## Context

Many tests write a stand-in program, a shell script posing as `gh`, `tmux`,
`claude`, `wt`, or `conspectus`, and run it or put it where the code under
test runs it. Each wrote the file with `fs::write`, set mode 0755, and ran
it. Under `cargo test` the tests of one binary run as threads of one
process, and that pattern fails now and then:

1. A test opens the new program for writing.
2. Another test's thread starts a child. The fork copies every open
   descriptor, the write descriptor included, and the child holds it until
   its own `exec` closes the close-on-exec ones.
3. The first test closes its descriptor and runs the program. The kernel
   refuses to execute a file that any process holds open for writing, so
   the `exec` fails with `ETXTBSY` ("Text file busy", os error 26).

The failures were in `discovery::forge` (`failed to spawn gh binary ...
Text file busy`, or an empty host list where the code under test maps the
failed start to "no answer") and in `rebuild::script_tests`. CI's
`cargo test` step failed on `main` this way, and passed on a rerun. Run
200 times under load with 16 test threads, the library's tests that write
programs failed in 29 runs. `just check` runs nextest, one process per
test, which never shows the race. The live poller's tests had worked
around it by retrying a failed start up to five times, which also let a
test that expects a failure pass on a start-up failure.

Conspectus itself never writes a program it runs, so the race is a property
of the tests, and the fix belongs there.

## Decision

1. **One helper writes every program a test runs.**
   `tests/support/program.rs` has `write_program(path, content)` and
   `copy_program(from, to)`. `write_program` has a `/bin/sh` child run
   `printf '%s' "$2" > "$1"`, so the write descriptor exists only in that
   short-lived child, from which no test thread forks; `copy_program` runs
   `cp` the same way. Both then set mode 0755 by path, which opens no
   descriptor. A process's descriptors are closed before its parent can
   reap it, so when the call returns nothing holds the program open for
   writing.
2. **Both test trees share it.** The library includes the file as
   `crate::test_program` under `#[cfg(test)]`; an integration test
   includes it with `#[path = "support/program.rs"] mod program;`, as the
   source-scan plumbing is shared.
3. **Each program gets its own path.** A test that needs a second stand-in
   writes it beside the first instead of over it.
4. **A scan enforces the rule.** `tests/program_hygiene.rs` fails on test
   code that passes a mode literal with an execute bit to `from_mode`,
   `mode`, or `set_mode` anywhere but the helper. Test code is every file
   under `tests/`, the unit-test files, and the test module that ends a
   source file. A test that changes a directory's permissions keeps the
   permissions it found and restores them, or masks them, so the rule has
   no exceptions. The same file checks the helper's promise: 100 programs
   it writes all start while four threads keep forking. The same check
   with `fs::write` failed in every one of five runs.
5. **No retry hides a start-up failure.** The live poller's retry is gone.

## Consequences

- The 200-run loop that failed 29 times before the change fails none after
  it, and CI's `cargo test` step no longer fails on this race.
- Writing a program costs a `/bin/sh` start, a millisecond or two, which no
  test notices.
- A test that writes a program the old way fails the scan with a message
  that names the helper, so the convention is learned at the point of
  breaking it. The scan reads literals; a mode computed at run time slips
  past it, a gap accepted to keep the scan simple.
- The helper needs `/bin/sh` (it is a shell builtin's `printf` that writes,
  so `PATH` does not matter) and, for `copy_program`, a `cp` on `PATH`.

## Alternatives Considered

- **Retry the start on `ETXTBSY` in the test.** Most tests hand the
  program to production code, which starts it and reports a failure its
  own way, so the test cannot see the error to retry on. Retrying the whole
  test hides real failures, as the live poller's retry did.
- **Write to a temporary name and rename into place, after syncing.** The
  inherited descriptor names the same file, so the program is just as
  busy under its new name.
- **A process-wide lock taken by every write and every spawn.** It would
  have to cover each child the test process starts, including those
  started inside production code and libraries, which a test helper
  cannot reach.
- **Run only nextest.** One process per test hides the race but does not
  remove it, and CI keeps `cargo test` to catch exactly this kind of
  shared-process failure (ADR 0224).
- **Retry or special-case `ETXTBSY` in production spawning.** Production
  never writes the programs it runs, so it cannot meet the race; changing
  it for the tests' sake would hide a real failure from operators.
