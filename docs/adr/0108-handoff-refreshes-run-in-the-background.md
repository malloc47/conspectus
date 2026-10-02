# ADR 0108: Hand-Off Refreshes Run In The Background

## Status

Accepted. Amends ADR 0104 (the TUI rescans live classes after tmux
hand-offs).

## Context

ADR 0104 made the refresh after a tmux hand-off ask the daemon to
rescan the `mux` and `harness` classes before taking its snapshot. That
fixed stale state, but the refresh ran on the UI thread. After an
attach returned, the TUI cleared the screen, sent two class nudges,
fetched and decoded the snapshot, and rebuilt the row tree, all before
drawing a frame. The screen stayed blank the whole time.

The nudges take about 0.2 s each on the dev tree. The daemon runs them
under its writer lock, so a nudge also waits behind any git or forge
rebuild already in progress. With no daemon, the same blocking path
runs a full local discovery, which takes seconds. Most returns are a
plain detach to switch to another harness. For those, almost nothing
on screen changes, and the operator waits anyway.

## Decision

1. **Paint first, refresh behind.** When an attach returns (attach,
   pin launch, `mux new`, `mux launch`), the executor dispatches
   `Msg::HandoffReturned(mux)` instead of refreshing. The next frame
   draws the snapshot the TUI already holds. The live loop sees the
   request and spawns the same nudged background worker that `r` uses
   (ADR 0104). It spawns it even when another worker is running,
   because that worker started before the hand-off.
2. **Mark what may be stale.** Until that refresh lands, the
   hand-off's mux row and the rows of agent sessions with an active
   `LinkedToMux` candidate to it render dimmed. A spinner sits in the
   attach-glyph cell, which is the value most likely to be wrong after
   a detach. A failed refresh also settles the mark; the existing
   stale-snapshot marker (ADR 0105) takes over. The rest of the UI
   stays live: the operator can move, filter, and attach again while
   the refresh runs. A second return before the refresh lands adds its
   mux to the same mark.
3. **Recapture the preview right away.** The hand-off drops the mux's
   cached pane capture. The selection pass then recaptures it on the
   same loop iteration, which costs one `tmux capture-pane`.
4. **Results apply in spawn order.** Each worker gets a generation.
   A result older than the one on screen is dropped, so a timer
   refresh that started before the hand-off can't overwrite the
   post-hand-off snapshot. The first result at or after the hand-off's
   generation settles the mark. The status-bar spinner stays up while
   any worker runs.

The refreshes that run before an attach (after `pin launch`,
`mux new`, `mux launch`) and after renames stay blocking. The UI isn't
waiting on a return from tmux in those cases.

## Consequences

- Returning from tmux shows the TUI right away, whether a daemon is
  running or not.
- For a fraction of a second (seconds without a daemon), the marked
  rows show pre-hand-off values. They're dimmed, so it's clear they
  aren't current. Rows outside the mark can also be one tick old; that
  was already true between timer refreshes.
- When a killed session's refresh lands, its row disappears and the
  selection moves to the nearest row, as with any refresh that removes
  a row.
- Up to two workers can run at once (one timer or `r` worker plus one
  hand-off worker). Each extra one is a daemon round trip, or a local
  discovery without a daemon.

## Alternatives Considered

**Guess the post-detach state in the graph.** The TUI could set the
mux's `last_attached_epoch` and client-attached flag itself and
re-sort before the refresh lands. That gets the common case right
sooner, but it writes guesses into the snapshot, and on a kill or a
pane exit the guess is wrong. Dimming makes no claim about the new
values, and the real values usually arrive within half a second.

**Make the nudge itself faster.** A single daemon call that rescans
`mux` and `harness` together would save one round trip and one resolve.
Faster tmux scans would also help. Both are still worth doing, but
neither removes the blocking wait. In particular, neither avoids
waiting behind the daemon's writer lock during a long git rebuild.

**Drop the nudge after a detach.** Without it, the background refresh
would show the daemon's last tick, which is the stale state ADR 0104
fixed.
