# ADR 0104: The TUI Rescans Live Classes After Tmux Hand-Offs

## Status

Accepted. Extends ADR 0082 (continuous server) for TUI clients.
Amended by ADR 0108: after an attach returns, this refresh runs in the
background, and the TUI dims the affected rows until it lands.

## Context

With `conspectus serve` running, every TUI refresh takes the daemon's
resolved snapshot (ADR 0082). The daemon rescans each provider class
on its own interval, so the snapshot can be a whole interval behind.

That's fine for timer refreshes. It's wrong right after the operator
changed tmux through Conspectus: attached and detached, launched a pin,
created a mux, or renamed a session. The TUI refreshes at exactly that
moment, gets the daemon's pre-hand-off snapshot, and shows the old
state. A session that was just created is missing, a mux that was just
renamed still has its old name, and attach markers are wrong. Pressing
`r` didn't help, because `r` also only took the daemon's snapshot.

The daemon already serves per-class refreshes (`refresh` with
`class`), the same operation `conspectus refresh --class mux` uses.
Rescanning the `mux` and `harness` classes takes about 0.2 s each on the
dev tree.

## Decision

- After every tmux hand-off or tmux mutation the TUI performs (attach,
  pin launch, `mux new`, `mux launch`, session and alias renames), it
  asks the daemon to rescan the `mux` and `harness` classes, then
  takes the snapshot.
- `r` does the same on its background worker. Timer refreshes don't
  nudge the daemon; they keep reading the daemon's last tick.
- Git and forge classes are not part of the nudge. Tmux hand-offs
  don't change them, and they are the slow classes.
- With no daemon running, nothing changes: the refresh runs discovery
  itself. If the daemon is reachable but the class refresh fails, the
  TUI rebuilds locally (as `--refresh` does) so the operator still sees
  current state, and reports the daemon failure instead of hiding it.

## Consequences

- The state after a hand-off is current, not one daemon interval old.
- Returning from tmux, or pressing `r`, costs about half a second more
  with a daemon up.
- Each nudge is an extra daemon rebuild of two classes. The daemon
  serializes rebuilds under its writer lock, so a nudge waits for any
  rebuild already running.

## Alternatives Considered

**Always rebuild locally after a hand-off.** This is always fresh, but
it costs a full cold discovery (seconds) where the daemon needs two
class rescans. It also leaves the daemon stale, so the next timer
refresh would show the old state again.

**Shorter daemon intervals.** A shorter interval narrows the window
without closing it, and it costs CPU all the time to fix a moment the
TUI can identify exactly.

**The daemon watches tmux for changes.** tmux has hooks and control
mode, but subscribing to them is a larger change to the daemon's
scheduler. That's worth revisiting for changes made outside
Conspectus. This ADR covers the hand-offs the TUI makes itself.
