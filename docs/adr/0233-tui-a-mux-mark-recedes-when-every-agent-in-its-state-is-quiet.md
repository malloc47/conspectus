# ADR 0233: A Mux's Mark Recedes When Every Agent In Its State Is Quiet

## Status

Accepted. Carries out the operator's decision of 2026-11-13 on the
question [ADR 0209](0209-tui-session-and-mux-rows-show-live-state-in-a-one-cell-column-after-the-age.md)
left open. Amends ADR 0209 decision 3 (a mux's mark was never dimmed)
and [ADR 0213](0213-tui-the-detail-pane-says-what-a-nodes-agents-are-doing.md)
decision 7 (the `agents` field's mark was "never dimmed, as on the
row"; it follows the row, as it always did). Adds no theme role (ADR
0153), no glyph (ADR 0154), no key, no effect, no write (ADR 0143), and
no snapshot field.

## Context

ADR 0209 draws a quiet agent's mark (work with no news past its source's
quiet window) with the muted look, and a mux's mark never, because a mux
row carried only per-state counts (`LiveCounts`) and the counts do not
say how fresh each agent is. A mux whose only working agent has gone
quiet therefore showed a bright `»` while that agent's own row, nested
under the mux, showed it dimmed: the rolled-up mark read more alive than
anything behind it.

## Decision

1. **The mark recedes with its agents.** A mux's mark is drawn with the
   muted look when every agent counted in its dominant state is quiet,
   and bright while any agent in that state is fresh. The dominant state
   is the one the mark already shows: the first counted by the model's
   default precedence (waiting, error, ready, working, then compacting,
   starting, unknown, exited). Agents in other states do not count: a
   quiet working agent beside a fresh compacting one still dims the `»`
   when working is the state that ranks first. The shape and the color do
   not change, as for a session.

2. **Only work goes quiet.** `working`, `compacting`, and `starting`
   are the states a source's quiet window can reach (ADR 0203); a wait, a
   finished turn, and an error never read quiet while the process lives,
   so a mux whose dominant state is one of them is never dimmed. A
   `starting` agent silent past its two-minute window reads stale, not
   quiet.

3. **The mux row carries a quiet count per state.** `MuxLive` gains
   `quiet`, a `LiveCounts` of the agents in each state that are quiet,
   filled by the same pass that fills `counts` and `unseen`
   (`LiveValues::at`), from the same list of agents that count toward the
   mux (`GraphSnapshot::quiet_counts_by_mux` beside `live_counts_by_mux`).
   The overlay's one-second check already notices an agent crossing its
   quiet window (the cells it compares carry the confidence), so the rows
   refill in place, with no rebuild, the way they do for any other
   time-driven change. The snapshot, `graph.bin`, and the JSON are
   unchanged.

4. **One sign everywhere.** `LiveSign::of_mux` is the only place the rule
   lives, so the Mux view's rows, a stopped mux's row, a mux pin's row,
   and the detail pane's `agents` field (ADR 0213 decision 7) draw the
   same mark in the same style.

## Consequences

- A mux whose agents are all quiet no longer reads as busy as one whose
  agents are working: the operator sees at a glance which running muxes
  have gone silent. The count of how many are quiet is not shown; opening
  the mux shows its agents' own marks.
- A mux with one fresh working agent and any number of quiet ones stays
  bright: it is working.
- In the showcase, `project`'s `»` is dimmed: both its working agents
  (the hook session and an agent in a pane no session matches) have been
  silent for eleven minutes or more, and a newer agent starting beside
  them does not keep it bright (starting ranks after working).
- A mux whose dominant state is waiting, ready, or an error never dims,
  whatever quiet work runs beside it.

## Alternatives Considered

- **Dim when every agent of the mux is quiet, whatever its state.** A
  waiting agent beside quiet work would be dimmed with it, or the rule
  would need an exception; the mark names one state, so the rule reads
  that state's agents.
- **Dim when any agent is quiet.** One stale agent would recede a mux
  that is otherwise working.
- **Dim when most agents are quiet.** A threshold the operator has to
  learn, for a mark one cell wide.
- **Keep the mark bright.** The decision of ADR 0209; the operator chose
  to change it.

## Open Questions Answered

- *Does the quiet count live in the snapshot?* No: it depends on the time
  it is read at, as the cells' confidence does, so it is derived each
  time the rows are filled.
- *Does the legend or the key change?* No: the help's "Agent state"
  legend already says a quiet mark is dimmed.
