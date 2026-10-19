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
4. **In-flight chips lead the status bar.** The chips used to trail
   the hint text, which is long enough to push them off the right
   edge of any terminal under about 190 columns, so the first version
   of this change showed no spinner on a typical terminal. They now
   sit right after the unseen-messages chip (ADR 0105), and they also
   render while a status message replaces the hints. This moves the
   discovery chip too.
5. **A pin launch marks its own row.** The launching pin's mux row
   (its placeholder, until the session exists) and the sessions
   linked to it get ADR 0108's dim-and-spinner treatment from the
   keypress, so the row being acted on looks the same as one the
   operator just detached from. `Msg::LaunchStarted` sets the mark and
   asks for no refresh. The attach's `HandoffReturned` then asks for
   one and keeps the mark until it lands. A launch that ends without
   an attach (subprocess failure, attach target missing, session not
   attachable) sends `Msg::LaunchAborted`, which drops only that mux's
   mark. `mux new` and `mux launch` create a session with no existing
   row, so they use the chip alone.
6. **Order is unchanged.** The refresh still finishes before the
   attach, so the failure checks and log entries read the same state
   as before.

## Consequences

- The operator sees the TUI and an animated chip from the keypress to
  the attach, instead of a shell screen and then a blank one.
- Keys typed during the launch stay queued in the terminal and are
  handled after the attach returns, as before.
- A launching pin's row carries the same mark as a row the operator
  detached from, so the two read the same. `mux new` and `mux launch`
  have no row to mark; the chip is their whole indication.
- A launch no longer clears the screen between the subprocess and the
  refresh.

## Alternatives Considered

**Chip only, no row mark.** The first version did this. The chip
covers all three launch kinds, but state about a row belongs on the
row, and the operator expects a launch to look like a detach. A
pin's placeholder mux row already exists with the id the launch will
create, so the hand-off mark applies as is.

**Draw one frame, then block.** Cheaper, but the spinner would be
frozen for the whole wait, which reads as a hang.

**Move the pre-attach refresh to the background, as ADR 0108 did for
returns.** The attach already triggers a background refresh when it
returns, so this refresh is largely redundant. But it also feeds the
launch log entries and the attach-availability check, so removing it
changes more behavior than this fix needs. Worth revisiting separately.
