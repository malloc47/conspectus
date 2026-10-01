# ADR 0105: TUI Message Log For Operation Outcomes

## Status

Accepted.

## Context

The TUI reported the outcome of an operation in two places. Neither
held on to it:

- **The status bar** shows one message, and the reducer clears it on
  the next selection or focus change. Moving the cursor erases it.
- **Toasts** expire on a timer.

Both show one line. Subprocess launches (`pin launch`, `mux new`,
`mux launch`) capture the child's full stdout and stderr, but the TUI
kept the first non-empty stderr line, cut to 180 characters, and threw
the rest away along with the exit status. An attach that failed showed
"tmux attach-session exited with exit status: 1". Other failures (a
rename that tmux refused, a worktree backend error, a daemon refresh
failure) got the same treatment.

The `conspectus-worker1` launch failure (ADR 0103) was the worst case:
a toast reading "can't find session" flashed past, and the cause was
nowhere the operator could read it again.

## Decision

1. **Every operation outcome is a log entry.** `App` keeps an
   in-memory `MessageLog` of the last 200 entries. Each entry carries a
   time, a level (`info`, `warning`, `error`), a one-line summary, an
   optional target (the pin or mux it concerns), an optional command
   record (argv, exit status, full stdout and stderr), and optional
   detail text. Launches, attaches, viewers, renames, pin-store and
   worktree operations, daemon nudge failures (ADR 0104) and refresh
   failures report through `Msg::Report`. Successes are logged too, so
   the log shows what happened as well as what failed. Navigation
   hints still use the plain status message.
2. **Messages overlay.** `!` opens a navigable overlay. It lists
   entries newest first, and a detail pane shows the selected entry's
   full record. `j`/`k` select, `J`/`K` and PgUp/PgDn scroll the
   detail, `y` copies it via OSC 52 (ADR 0056), `Esc`/`q`/`!` close.
   The help overlay lists it under "Discoverable controls".
3. **Unseen failures stay visible.** A warning or error sets the status
   message to its summary plus "· ! details". Until the overlay is
   opened, the status bar also keeps a persistent "⚠ N · ! messages"
   chip, so a failure is still discoverable after its status message
   has been cleared.
4. **Failures show next to their row.** When the selected row's pin or
   mux has a warning or error as its latest log entry, the row's
   Preview starts with that failure: summary, exit status, and the
   last lines of stderr or pane output. A later successful operation on
   the same target replaces it.
5. **After an attach returns, check the pane.** When an attach or pin
   launch returns and the target's pane is dead (ADR 0103 keeps failed
   panes), the TUI logs an error with the pane's text, so a harness
   that failed while the operator was away is reported.
6. **Nothing is persisted.** Pane output and harness stderr can contain
   conversation content. The log is in memory only, for the life of
   the TUI process, which makes it a Tier 2 read surface (ADR 0086).
   Launch outcomes still print to stderr in the CLI.

## Consequences

- A launch failure can be read in full after the fact, from the
  overlay or from the row it concerns.
- Status-bar messages still clear on navigation. The chip and the log
  are what keep failures around.
- The log is lost when the TUI exits. Errors that need to outlive the
  process belong in the CLI, which prints them.
- Reporting code goes through one entry point, so a new operation
  only has to build a `LogEntry`.

## Alternatives Considered

**Keep the full output only in the Preview.** This ties each error to
its row, but the error disappears when the row does, for example when
a failed `mux new` leaves no row. There's also no history, and
failures without a row (daemon, refresh, store writes) have nowhere to
go. The Preview banner is kept as a second surface.

**Make status messages sticky until dismissed.** A sticky status bar
hides the contextual hints the bar exists for, and it still holds one
line of one event.

**Write the log to `$XDG_STATE_HOME`.** That would let the log survive
restarts and help with bug reports, but it would persist pane output,
which can be payload (ADR 0086 prohibition 3). A redacted on-disk log
could be a separate decision.

**An error-only log.** Operators asked what happened after a hand-off,
not only what failed, and successes give the failures context ("the
resume failed, then a fresh launch worked").
