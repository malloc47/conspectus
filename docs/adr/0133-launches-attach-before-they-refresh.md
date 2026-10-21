# ADR 0133: Launches Attach Before They Refresh

## Status

Accepted. Amends ADR 0103 (item 2, the watch after a fresh launch) and
ADR 0127 (item 6, the refresh that ran before the attach).

## Context

Pin launch, `mux new`, and `mux launch` create a detached tmux session,
and the harness inside starts at once. The operator attached only after
two more steps:

1. The launch subprocess watched the new pane for a failed start: 0.5 s
   after a fresh launch, 1.5 s after a resume (ADR 0103).
2. The TUI refreshed before attaching: two daemon nudges of about
   0.2 s each, which can also wait on the daemon's writer lock, or a full
   local discovery without `conspectus serve` (ADR 0127).

The harness ran through both, so by the time the operator attached, its
first output had already scrolled past. The operator noticed the delay
even with the daemon running.

ADR 0127 already called the pre-attach refresh largely redundant: the
attach triggers a background refresh when it returns (ADR 0108). The
attach target is resolved before the launch, and the attach-availability
check asks tmux directly, so neither needs the refreshed snapshot.

The fresh-launch watch only catches failures in the harness's first
half second, such as a missing binary. ADR 0103's retained panes
already keep those visible: a harness that exits non-zero leaves a dead
pane with its output, which the attach shows and the TUI's
attach-return log entry reports.

## Decision

1. **A successful launch attaches straight away.** Once the launch
   subprocess succeeds, pin launch, `mux new`, and `mux launch` attach
   without refreshing first. The attach's `HandoffReturned` drives the
   background refresh, as for any attach (ADR 0108). The launching
   row's dim-and-spinner mark (ADR 0127) stays on until that refresh
   lands.
2. **A launch that ends without an attach still refreshes first.**
   When the subprocess fails, the pin has no attach target, or its
   tmux session can't be attached, the TUI runs the blocking refresh
   behind the spinner before reporting, as before. The launch may
   still have changed state (a dead session removed, a worktree
   created).
3. **A fresh launch isn't watched.** `pin launch` and `mux launch`
   return once `tmux new-session` succeeds. This includes the fresh
   launch that replaces a failed resume.
4. **A resume is still watched for 1.5 s.** The watch drives the
   fresh-launch fallback and clears the resume sidecar, which only a
   watch can do.

## Consequences

- The operator attaches within the time of a `has-session` and an
  attach after the subprocess returns, and sees the harness's output
  from its start (or 1.5 s in, for a resume).
- A fresh launch that dies at once no longer fails the launch command.
  If it exits non-zero, the operator attaches to the dead pane, sees
  the harness's error and tmux's "Pane is dead" banner, and the TUI
  logs it as an error when the attach returns. The next pin launch
  replaces the dead session (ADR 0103 item 4). A `mux launch` with the
  same name fails until the operator closes it.
- A fresh launch that exits 0 at once ends its session, so the attach
  finds nothing. The CLI's error for that now says the process may
  have exited, rather than blaming a race.
- `pin launch --no-attach` and `mux launch --no-attach` in scripts no
  longer exit non-zero when the harness dies right after a fresh
  launch. The dead pane is still there to inspect.
- After the attach returns, the TUI briefly shows the tree from before
  the launch, with the launched rows marked, until the background
  refresh lands. `mux new` and `mux launch` rows appear when it does.

## Alternatives Considered

**Hold the harness until a client attaches.** Starting the session with
a command that waits on a tmux `wait-for` channel, released by a
`client-attached` hook, would show every byte of output. It adds tmux
subcommands and a hook to the capability manifest (ADR 0117) and the
mutation envelope (ADR 0087). It would also turn the resume watch into
a watch after the attach. Worth it only if the remaining gap matters.

**Shorten the fresh window instead of removing it.** A missing binary
fails within milliseconds, but any window still delays the attach, and
a failure the window misses already shows as a dead pane.

**Keep the fresh watch for the CLI and skip it for the TUI.** The TUI
launches through the CLI's `--no-attach` subprocess, so this would need
a new flag for a difference operators wouldn't expect.
