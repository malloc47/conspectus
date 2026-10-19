# ADR 0127: Launches Keep The TUI Drawn Behind A Spinner

## Status

Accepted. Amends ADR 0108 (the refreshes that run before an attach).

## Context

ADR 0108 made the refresh after an attach returns run in the
background, with dimmed rows and a spinner until it lands. It left the
steps that run *before* an attach blocking, on the grounds that the UI
isn't waiting on a return from tmux. Those steps are the whole wait
when the operator starts something that doesn't exist yet: Enter on an
unstarted pin, `mux new`, or `mux launch`:

1. The TUI left the alt screen and re-executed
   `conspectus ... --no-attach`, which runs its own discovery and
   `tmux new-session`. The terminal showed whatever was in the shell's
   scrollback.
2. The TUI re-entered the alt screen and cleared it, then ran the
   blocking refresh: two daemon nudges (about 0.2 s each, behind the
   daemon's writer lock), or a full local discovery with no daemon.
   The screen was blank.
3. Only then did the attach start.

Together that is a pause of up to several seconds with nothing on
screen to say the launch is happening.

## Decision

1. **Keep the TUI up for the launch subprocess.** The subprocess's
   stdout and stderr are captured and its stdin is null, so it never
   needs the terminal. The TUI no longer leaves the alt screen for it.
   The terminal is only handed over for the attach itself.
2. **Run both blocking steps on a worker thread behind a status-bar
   spinner.** `run_with_spinner` starts the work on a thread and
   redraws the TUI every 50 ms until it finishes, with an
   `InFlightKind::Launch` chip: `Launching <name>` for the subprocess
   (`Creating <name>` for `mux new`), then `Refreshing` for the nudge
   and reload. The chip uses the existing in-flight spinner (the same
   glyph and cadence as the discovery chip). A worker panic propagates
   to the caller.
3. **The refresh is split into a fetch and an apply.** The daemon
   nudge and snapshot load touch no UI state, so they run on the worker
   (`fetch_after_mux_handoff`). The snapshot is applied to the app on
   the UI thread afterwards (`apply_handoff_refresh`). The synchronous
   `refresh_after_mux_handoff` (used by renames) is the same two steps
   run back to back.
4. **Order is unchanged.** The refresh still finishes before the
   attach, so the failure checks and log entries read the same state
   as before.

## Consequences

- The operator sees the TUI and an animated chip from the keypress to
  the attach, instead of a shell screen and then a blank one.
- Keys typed during the launch stay queued in the terminal and are
  handled after the attach returns, as before.
- The row for an unstarted pin is not marked: unlike a hand-off, there
  is no existing mux row to dim, so the chip carries the whole
  indication.
- A launch no longer clears the screen between the subprocess and the
  refresh.

## Alternatives Considered

**Dim and spin the pin's row, as hand-offs do.** The row is the right
place for state about that row, but a launch has no mux row yet, and
the chip already covers all three launch kinds. A row marker can be
added later without changing this mechanism.

**Draw one frame, then block.** Cheaper, but the spinner would be
frozen for the whole wait, which reads as a hang.

**Move the pre-attach refresh to the background, as ADR 0108 did for
returns.** The attach already triggers a background refresh when it
returns, so this refresh is largely redundant. But it also feeds the
launch log entries and the attach-availability check, so removing it
changes more behavior than this fix needs. Worth revisiting separately.
