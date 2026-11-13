# ADR 0224: Local Checks Run The Workspace Tests Once, Through Nextest

## Status

Accepted. Supersedes the part of [ADR 0007](0007-rust-development-approach.md)
that made both test runners local baseline checks: `cargo nextest run` is
the local run, and `cargo test` stays a CI check. Rests on
[ADR 0099](0099-caller-owned-discovery-caches.md), after which "test
isolation no longer depends on process isolation."

## Context

`just check` ran `cargo test --workspace --all-targets --all-features` and
then `cargo nextest run` with the same flags, so every workspace test ran
twice. On 2026-11-12 that cost 101 s for `cargo test` and 75 to 85 s for
nextest after a one-line edit. On 2026-11-13, with 5,681 tests and the dev
profile of ADR 0223, the `cargo test` step took 148 s (its build of the
edit included) and nextest 64 s, of a 306 s `just check`.

The two runners differ in how they run a test, not in which tests they run:

- `cargo test` runs one test binary at a time, and the tests of a binary as
  threads of one process. A slow binary holds up every binary after it: the
  capability trace (`tests/capabilities.rs`) takes about 33 s.
- nextest runs each test in its own process, across binaries at once.
- With `--all-targets`, neither runs doctests. The framework's doctests run
  in `just tuikit-features`, and still do.

What only `cargo test` catches is a test that fails when it shares a
process with other tests: process-global state, environment variables, the
working directory, or a file a sibling test's child inherits. The known
`Text file busy` flake is one: a test writes a stub executable while
another test's thread forks, and the child holds the file open for writing.
It failed the first baseline run for this change.

ADR 0007 listed both runners among the baseline checks. When it was
written, process-global discovery caches made the two behave differently;
ADR 0099 moved those caches to their callers and concluded that plain
`cargo test` and nextest behave the same. The operator accepted the
remaining trade-off on 2026-11-12: a test that fails only in a shared
process is caught by CI before `main`, not by the local check.

## Decision

1. **`just check` runs the workspace tests once, through nextest.** Its
   `check` recipe lists `nextest` and no longer lists `test`.
2. **`just test` stays.** It runs `cargo test` with the workspace flags,
   for reproducing a failure of CI's `cargo test` step.
3. **CI runs both.** The `Run tests` (`cargo test`) and `Run nextest` steps
   of `.github/workflows/ci.yml` stay, on every push and pull request.
4. **The docs say so.** The pre-main invariants in `AGENTS.md`, the dev
   shell's banner, and the README's development section name nextest as
   the local run and `cargo test` as a CI run.

## Consequences

- `just check` after a one-line edit is shorter by the `cargo test` step:
  233 s instead of 306 s on 2026-11-13, at a load of 6 rising to 20. The
  nextest step now includes the build of the edit (119 s in all).
- A test that fails only when it shares a process fails CI's `cargo test`
  step instead of the local check. The `Text file busy` flake no longer
  fails local checks, since nextest gives each test its own process; it can
  still fail CI's `cargo test` step, as it could before.
- ADR 0007's list of baseline checks remains the list CI runs.

## Alternatives Considered

- **Keep both runners local.** The cost above, on every check, for a class
  of failure CI still catches.
- **Run `cargo test` locally and nextest only in CI.** `cargo test` is the
  slower runner, since its binaries run one after another, and its shared
  process is the less faithful model of how the tests now isolate their
  state.
- **Drop `cargo test` from CI as well.** The operator chose to keep every
  check; CI is where the shared-process run delays no one.
- **Run `cargo test` in a commit or push hook.** The same CPU on the same
  machine, at a moment the agent waits for anyway, and a hook to maintain.

## Open Questions Answered

- *Does `just check` still predict CI?* For every failure except one that
  needs a shared process, yes.
