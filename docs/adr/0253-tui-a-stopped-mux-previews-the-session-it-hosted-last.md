# ADR 0253: A Stopped Mux Previews The Session It Hosted Last

## Status

Accepted. Amends [ADR 0112](0112-node-specific-previews.md) (decision 1,
the `MuxSession` row of the table) for a mux that is no longer live, and
refines the gone-node rule of [ADR 0140](0140-history-and-presence.md).
No new key, setting, or write path.

## Context

A mux that has stopped is a ghost in the graph: history projects its node
with a `gone` presence, and a `mux_last_hosted` link names the session it
hosted last when the graph still holds that session. ADR 0112 gave a mux
its pane capture, and ADR 0140 left a gone node with no source, since it has
no pane, repository, or directory to read. So a stopped mux, pinned or only
remembered, had nothing to preview. The Mux view said `no mux target for
this row` and the History view said `no preview for this row`, directly
under a `last hosted` row that named the session. The operator (2026-11-24)
asked for that session's preview in their place.

## Decision

1. **A mux that is not live previews what the session it hosted last
   previews.** The session is the target of the mux's `mux_last_hosted`
   link, the most recently active one if there are several. The Preview
   resolves it exactly as that session's own row would: its transcript, or
   its live pane under `[tui] session_preview = "pane"` when the session
   now runs in another mux, and hidden by `--no-live-preview`.
2. **The same mux previews alike wherever it appears.** The Mux view's
   ghost rows, the History view's mux rows, and a mux node reached by
   drilling in from a Related row all resolve to that source.
3. **Without a session to show, the placeholder stays.** A mux with no
   `mux_last_hosted` link, whose session the graph does not hold, or whose
   session is gone too, says what it said before.
4. **Nothing else moves.** A running mux previews its pane, and none when
   it hosts this TUI. A Pinned view row for a stopped pin keeps its summary
   of the pin's status and the `rebuild` command, since that row is a pin,
   not a mux.

## Consequences

- The conversation a stopped mux was last used for is one cursor move away
  in every view that lists it, which is what the operator wants when
  deciding whether to rebuild it or forget it.
- The transcript appears under the mux's own fields, with the `last hosted`
  row above it naming whose it is. The Preview has no heading of its own.
- A mux that last hosted a session now running elsewhere previews that
  session as it is now, not as the stopped mux had it.
- Only a live session is shown. A session that is itself gone has no
  transcript source, so its mux shows the placeholder, as before.

## Alternatives Considered

- **Show the pin summary or a note beside the transcript.** The Pinned view
  keeps its summary because its row is the pin. On a mux row the fields and
  the `last hosted` row already say what the mux is, and a second header
  would crowd a Preview with little room.
- **Read the session from the history record, not the link.** It would
  work without projection, but the `last hosted` Related row and the Pinned
  view's session rows read the link, and a Preview that disagreed with the
  row above it would be confusing.
- **Also replace the Pinned view's pin summary.** The summary says why the
  pin has nothing live and what Enter does; replacing it would hide that.
  Left for the operator to ask for.

## Open Questions Answered

- **Which session, when a mux hosted several?** The one with the newest
  activity; the link carries one session in practice, since history keeps
  the last.
- **Does previewing it mark it read?** No. Unread marks are set by their
  own actions, and a Preview only reads.
