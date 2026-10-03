# ADR 0098: Pin Resume Preserves the Pin's Launch Argv

## Status

Accepted

Amends ADR 0058 (pin session continuity), step 4 of the launch
decision.

## Context

ADR 0058 lets `pin launch` resume the pin's last-bound harness
session. Step 4 of its launch decision reads: "Consult
`HarnessAdapter::resume_argv(session_id, cwd)` … If `Some(argv)` →
use that argv with `tmux new-session`." The implementation took this
literally and **replaced** the pin's argv with the adapter's bare
resume argv (`claude --resume <id>`, `codex exec --resume <id>`).

That is only correct when the pin uses the harness default argv.
Pins routinely carry a configured `launch.argv` with:

- **Wrapper prefixes** — `atelier exec claude …`, `nono run -- claude
  …`, which apply sandboxing / workspace policy around the harness.
- **Launch options** — `--dangerously-skip-permissions` (the
  `skip permissions` toggle in the pin form), model flags, etc.

Observed failure (CSP-531): a pin with
`launch.argv = ["atelier", "exec", "claude",
"--dangerously-skip-permissions"]` was stopped and relaunched; the
sidecar had a recorded session, so tmux ran `claude --resume <id>`.
The pin edit form still showed the full argv and the skip-permissions
toggle as enabled, but the running agent had neither the wrapper nor
the flag. Silently dropping a sandbox wrapper is a correctness and
safety problem, not just a UX one.

## Decision

Resume tokens are **spliced into** the pin's effective launch argv
rather than replacing it.

1. The effective base argv is the pin's `launch.argv` when set and
   non-empty, else the harness `launch_argv` (unchanged from ADR
   0057).
2. The first token of `resume_argv` names the harness binary. Find
   the first base token whose file name equals that binary (so
   `/nix/store/…/bin/claude` matches `claude`).
3. Insert the remaining resume tokens immediately after that base
   token. Everything before it (wrappers) and after it (launch
   options) is preserved in order:
   `atelier exec claude --dangerously-skip-permissions` →
   `atelier exec claude --resume <id> --dangerously-skip-permissions`.
   Placing resume tokens directly after the binary also keeps
   subcommand-shaped resumes (`codex exec --resume <id>`) valid.
4. If no base token matches the binary (e.g. an opaque
   `./start-agent.sh`), Conspectus cannot splice honestly: it emits a
   status hint and **launches fresh with the pin's argv**, joining
   the other ADR 0058 fall-back-to-default-argv paths. The sidecar is
   kept — the session is still valid, only this argv can't carry it.

The splice is a pure, provider-neutral helper
(`discovery::harness::splice_resume_argv`); adapters keep returning a
full `resume_argv` so the standalone TUI resume action is unchanged.

## Consequences

- Pins with wrappers or options resume with the same launch shape
  they start fresh with. What the pin form shows is what runs.
- Default-argv pins produce exactly the same argv as before.
- A pin whose argv hides the harness behind a script loses automatic
  resume (with a visible hint) instead of losing its wrapper.
- The standalone TUI "resume session" action (`src/tui/resume.rs`)
  still runs the bare adapter resume argv; it has no pin argv to
  preserve. Carrying launch options there is a separate question.

## Alternatives Considered

- **Append resume tokens to the end of the pin argv.** Simple and
  works for `claude --resume`, but breaks subcommand-shaped resumes
  (`codex --flag exec --resume` is not valid) and puts resume tokens
  after a wrapper's `--` separator in the wrong position for some
  wrappers.
- **Adapter-level `resume_args` returning only the suffix.** Equivalent
  to the chosen splice but widens the adapter trait; the binary token
  already present in `resume_argv` is enough to locate the splice
  point.
- **Keep replacement, re-apply known launch options.** Would restore
  `--dangerously-skip-permissions` via `HarnessLaunchOption` fragments
  but still drops arbitrary wrappers and unknown flags.
- **Skip resume whenever `launch.argv` is customized.** Honest but
  throws away ADR 0058 continuity for most real pins.
