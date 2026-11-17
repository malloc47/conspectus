# ADR 0244: A Manual Driver Records Real Harness Sessions

## Status

Accepted. Follows [ADR 0242](0242-a-hook-trace-records-what-hook-write-receives-with-every-free-text-value-redacted.md)
(the trace the driver turns on) and [ADR 0243](0243-real-sessions-corrected-three-assumptions-the-hook-writers-made.md)
(what the first recording found). A development tool beside the code, like
`scripts/dev-env.sh` ([ADR 0226](0226-commands-load-the-dev-shells-environment-from-a-cache.md)):
it is not part of the `conspectus` binary, adds no dependency, and starts no
process on the binary's behalf, so [ADR 0143](0143-mutation-envelope-with-history-pins-and-clone.md)
and `docs/capabilities.toml` are unchanged.

## Context

The live-state reducers rest on what each harness sends its hooks, and a
harness that ships weekly makes that a guess until a real session confirms it.
Backlog CSP-822 did that once, by driving Claude Code, Codex, and Pi in a
private tmux server with the hook trace on, and found three assumptions wrong
(ADR 0243). The driver for it was throwaway shell in RAM-backed `/tmp`, gone
at the next reboot, and the recording is worth repeating only if the driver is
too.

Running a harness for real is not free of risk. It spends the model quota of
the account the harness is signed in to. And building the first driver
cost two sessions: a `script` process fed from `/dev/zero` grew to 7 to 9 GB in
seconds on a machine whose swap was full, was killed by the OOM killer, and,
because tmux pane scopes run with `OOMPolicy=stop`, systemd stopped the whole
scope with the session that had started it. A harness started from inside
another harness also inherits its session variables (`CLAUDE_PID`,
`CLAUDECODE`, `TMUX`), which corrupts what the hooks report.

## Decision

1. **The driver is `scripts/record/`, started by `just record HARNESS`.**
   One scenario script per harness (`claude-code`, `codex`, `pi`), a shared
   library, a README, and two small Python helpers (a reader for traces and the
   store, and a stand-in model gateway for recording `StopFailure` without a
   real failure). The scenarios reproduce the checks CSP-822 ran. OpenCode is
   not covered.
2. **It is manual, and cannot start by accident.** It is in no `just check`
   dependency, no CI job, and no test; a test checks that. It prints the quota
   it will use (about how many short prompts, to which account) and the
   directories it will create, and then needs `--yes` or the word `record`
   typed at a terminal; without a terminal it stops. `--dry-run` prints and
   stops.
3. **It runs in a sandbox of scratch.** A private tmux server with its own
   socket; each harness under `env -i` with a short list of variables; copies of
   the harness's configuration (read from the operator's, never written back,
   with the hook commands pointed at the `conspectus` the run uses, so the
   operator's installed link stays); a throwaway project; and a scratch
   `XDG_STATE_HOME`, so the operator's live-state store is untouched. Every
   process of a run carries a marker, so the cleanup ends all of them.
4. **It is bounded.** Each run is in its own `systemd-run --user --scope` with a
   memory cap (default 3G), no swap, `OOMPolicy=continue`, and a wall-clock
   limit; it refuses to run without `systemd-run`. No script gives a pty tool
   or anything else an unbounded input (no `script`, no `/dev/zero`, no
   `yes`); a test checks that.
5. **What it keeps.** The traces the writer already redacted (ADR 0242), a
   table of each, and screen snapshots and a timeline. The screens are not
   redacted, so they and the timeline go to a git-ignored directory
   (`record-out/`) and are never copied into the repository. A recording
   becomes a fixture by hand, under `tests/fixtures/hooks/recorded/`.

## Consequences

- A harness update can be checked against the reducers by running a scenario,
  and the recorded sequences compared with the fixtures.
- The tool needs Linux with `systemd --user`, tmux, and Python 3, and a
  `conspectus` built with the hook trace. It costs quota on every run.
- The scenarios match text the harnesses draw (a dialog's heading, a footer), so
  a harness's UI change can break a step. A step that drifts still records, and
  the README says how to write one that does not.
- The tests check the scripts parse, that none gives a pty tool an unbounded
  input, that nothing automatic starts them, and that a run without
  confirmation refuses.

## Alternatives considered

- **Leave the driver in `/tmp`.** The status quo; it was already lost once and
  holds the two hazards only in memory.
- **A `conspectus record` subcommand.** Would put quota-spending runs, tmux, and
  the harness processes inside the product, with a capabilities entry for each,
  for a task only a maintainer does. Rejected.
- **The harnesses' own non-interactive modes** (`claude -p`, `codex exec`).
  They fire hooks, but not the dialogs, interrupts, subagents, and shutdowns the
  reducers are about. Rejected.
- **An `expect`-style library.** A new dependency for what `tmux send-keys` and
  `capture-pane` already do. Rejected.
- **A scheduled CI run with a test account.** Credentials in CI and a standing
  bill, for a check whose value is a human reading the result. Rejected.

## Open questions answered

- *Should the recordings run in CI?* No.
- *Should screens be kept?* Not in the repository: unredacted.
