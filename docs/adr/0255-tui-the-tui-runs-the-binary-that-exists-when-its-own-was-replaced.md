# ADR 0255: The TUI Runs The Binary That Exists When Its Own Was Replaced

## Status

Accepted. Refines ADR 0117 for the program `conspectus` (`self` in the
manifest): which file the TUI starts when it runs Conspectus again. No new
program, argument, or write path.

## Context

The TUI carries out a rebuild, a new mux, and a launch by running
`conspectus <args>` as a child process, finding the binary with
`std::env::current_exe()`. On Linux that call reports a file that has been
replaced or removed as `<path> (deleted)`, and nothing has that name. A
`cargo build --release` replaces the binary a running TUI started from, so
every TUI left open across it failed on its next rebuild, new mux, or
launch with `failed to spawn: No such file or directory (os error 2)` and an
argv that ended in `(deleted)`. Nothing said why, or that restarting would
fix it. The operator (2026-11-24) hit it recreating a pin from a TUI started
three hours before the rebuild, while two newer TUIs worked.

## Decision

1. **The path itself, when a file is there.** The binary runs as it did. A
   file whose real name ends in ` (deleted)` is checked first and is not
   mistaken for a replaced one.
2. **Else the file now at that place.** When the path ends in the marker and
   a file is at the path without it, that is what runs. It is what a
   rebuild put there, which is also what a restarted daemon and a new CLI
   run, so the child and the daemon agree on the snapshot format.
3. **Else the running image.** When nothing is at that place (a `cargo
   clean`, a collected store path), the child runs `/proc/<pid>/exe`, which
   the kernel keeps runnable while the TUI does. It is the old version, so it
   is the last resort.
4. **Say so, once per run of a child.** Whenever 2 or 3 was used, the TUI
   posts a warning to the message log and a toast: the binary changed under
   the TUI, what ran instead, and to restart the TUI when convenient. The
   action goes on and reports its own outcome as before. The message log
   folds a repeat into one line.
5. **A spawn that still finds nothing says what to do.** `NotFound` from
   starting the child gains the reason: conspectus is gone from where the TUI
   started it, so restart the TUI.
6. **The decision lives in `exec`.** `exec::RunningBinary::locate` is the
   only place that reads `current_exe` to run Conspectus again, beside the
   other place that decides what to start. `hook init`, which records a path
   for later and runs in a short-lived process, keeps its own rules (ADR
   0240).

## Consequences

- A TUI left open across a rebuild keeps rebuilding pins, launching, and
  starting muxes, and the operator is told it is out of date instead of
  meeting an error.
- Order 2 before 3 runs a newer child than the TUI. That is the safe skew:
  the child speaks to the daemon the same operator restarted onto the new
  build, and the commands it runs are `rebuild`, `mux new`, and `mux
  launch`, whose interface the TUI passes unchanged. An old image reading a
  newer daemon's snapshot is the hazard order 3 accepts only when nothing
  else exists.
- The traced capability run starts the binary intact, so it never sees a
  fallback path. The manifest's `self` rule still means the running binary.
- On a system without `/proc/<pid>/exe` there is no marker to strip and
  `current_exe` never reports one, so only the intact path applies.

## Alternatives Considered

- **Always run `/proc/<pid>/exe`.** It keeps one TUI on one version, but that
  is the old one, against a daemon that is probably new: the mismatch the
  restart is meant to cure.
- **Strip the marker and say nothing.** The action works, and the operator
  never learns the TUI is stale, so the next surprise (a changed key, a
  format) has no explanation.
- **Warn as soon as the file changes.** A standing "update available" chip
  would need the TUI to watch its own file and gives a signal before any
  action needs it. Worth having, but a larger change than keeping launches
  working, and not what the error was.
- **Re-exec the TUI into the new binary.** It cures the staleness and throws
  away the operator's screen at a moment they did not choose.

## Open Questions Answered

- **Why warn on each use?** A toast fades and the status line changes with the
  next message, so a single warning at the first use is easy to miss. The log
  folds repeats, and the actions are the operator's own, rare ones.
- **Does it cover the hook installer?** No. It writes a path for a harness to
  run later, which ADR 0240 already chooses with its own checks.
