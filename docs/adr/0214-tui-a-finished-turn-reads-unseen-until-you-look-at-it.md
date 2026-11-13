# ADR 0214: A Finished Turn Reads Unseen Until You Look At It

## Status

Accepted. Carries out the operator's decision 6 in section 7.1 of the
live-state plan (`docs/plans/live-state/README.md`; the options are in
section 5 of `tui-options.md` there): seen and unseen for `ready`,
derived read-only, bold for unseen and plain for seen, the same shape,
nothing persisted. Amends
[ADR 0209](0209-tui-session-and-mux-rows-show-live-state-in-a-one-cell-column-after-the-age.md)
decision 2 (the ready mark gains a weight) and
[ADR 0213](0213-tui-the-detail-pane-says-what-a-nodes-agents-are-doing.md)
decisions 2 and 7 (the detail pane says which). Adds no theme role
(ADR 0153), no glyph (ADR 0154), no key, no effect, and no write
(ADR 0143).

## Context

A `ready` mark says the agent's turn is over and it takes a prompt. Six
of the tools surveyed split that into a turn the operator has seen and
one they have not, which is the strongest "needs you next" signal after a
wait. The model keeps no such fact: `since` says when the ready state
began, and a mux records when a tmux client last attached to it
(`#{session_last_attached}`) and whether one is attached now.

The decision: a ready turn is unseen until a tmux client has been
attached to its mux after the turn ended, or, for a session with no mux,
until the operator selects it in this TUI, in memory only. It left open
what counts as "attached after", how a refresh that loses the evidence
is handled (agent-deck's persisted flag flickers), what "selects" means
while the cursor scrolls past, how a mux's mark rolls it up, and how it
reads where weight does not show.

tmux updates a session's last attach time when a client attaches or
switches to it, not while a client stays attached, so a turn that ends
while the operator watches it in an attached client is after the last
attach time. The TUI's tmux listing is as old as its last refresh.

## Decision

1. **Seen through the mux.** An agent's mux is the session's preferred
   mux (the one its live state counts toward), or, for an agent in a
   pane no session matches, that pane's mux. Its turn that ended at
   `since` is seen when the mux's last attach time is at or after that
   second (tmux keeps whole seconds, so an attach in the turn's last
   second counts), or when a snapshot loaded after the turn ended lists
   a client attached to the mux.

2. **Seen by resting, with no mux.** A session the snapshot lists no
   mux for is seen once the cursor has rested on one of its rows (a
   session row in any view, a content search's hit under it, or a pin
   on it) for one second (`SEEN_REST`). A cursor that passes the row
   sooner, as key repeat does while scrolling, starts the rest again on
   the next row and sees nothing. Resting on a session that has a mux
   does not see it: its turn is seen by attaching.

3. **Seen stays seen, in memory.** The TUI keeps, for each agent, when
   its latest seen turn ended, and adds every turn its rows find seen
   each time they are filled (a snapshot put on screen, a refill from
   the live store). A later refresh that no longer lists the client
   attached cannot turn the turn unseen again, so there is no unseen,
   seen, unseen flicker; a turn that ends later is unseen until it is
   looked at. Nothing is written: the memory is not in the state file,
   and a new TUI starts without it.

4. **Weight, added.** An unseen `›` (and an unseen Pinned `ready`)
   draws bold in the ready mark's color; a seen one draws plain, as
   before. The shape and the color never change, so the distinction is
   added to a state that already reads without it (principle 2 of the
   reference document holds: the state is the shape; weight says only
   whether you have looked). On the cursor row, whose highlight is
   bold, the two look alike.

5. **A mux's mark follows its ready agents.** A mux whose mark is `›`
   draws it bold while any ready agent counted on it has an unseen turn.
   Its other marks are unchanged.

6. **The detail pane says it in words.** A ready session's `state`
   reads `ready · finished · unseen` or `ready · finished · seen`, each
   word previewing its glossary entry, and a mux's `agents` says `2 ready
   (1 unseen)` while some are unseen.

7. **View state, not an effect.** Judging and remembering are part of
   filling the rows, like a fold or the cursor: no effect is emitted, so
   nothing joins the effect lifecycle table, and no write path is added.

## Consequences

- Attaching through Conspectus sees a mux's turns when the refresh after
  the hand-off lists the client, which is when the row stops showing
  the hand-off's spinner.
- A turn that ends while the operator watches it in an attached client
  reads unseen until a refresh lists that client still attached (a timer
  refresh while the operator is there); if none comes before the client
  leaves, it stays unseen until the next attach. An agent that runs in
  the TUI's own tmux session reads seen at each refresh, since that
  session has a client while the TUI is on screen.
- In the showcase, the agent-deck launcher's turn is seen (its mux was
  attached after it ended) and the codex parent's, with no mux, unseen.
- `--snapshot` drives keys without the event loop's ticks, so a rest
  never completes there; its frames show the derived judgment only.

## Alternatives Considered

- **Persist seen turns** (a rebuildable observation). It would survive
  restarts, but it is a write the operator did not ask for, and the
  decision ruled it out.
- **The last attach time alone.** Simpler, but a turn watched in an
  attached client would stay unseen until the next attach.
- **Judge each refresh afresh, with no memory.** A refresh after a
  detach would turn a turn the operator saw unseen again.
- **Count a cursor that touches the row.** Scrolling through a list
  would see every turn it passed.
- **Count resting on a session in a mux too.** The decision ties those
  to attaching; the detail pane's transcript is not the agent's screen.
- **Dim the seen mark instead of bolding the unseen one.** It would
  read as quiet, which dimming already means.

## Open Questions Answered

- *Does a pin row count as the session's row for resting?* Yes: a pin
  on a session stands for it, and its preview is the session's.
- *Does a stopped mux's mark go bold?* By the same rule, if an agent it
  still counts is ready and unseen; one with no live mux has no attach
  to see it by.
