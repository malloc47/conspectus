# ADR 0231: A Turn Watched In An Attached Client Reads Seen When The Client Comes Back

## Status

Accepted. Carries out the operator's decision of 2026-11-13 on the
follow-up to [ADR 0214](0214-tui-a-finished-turn-reads-unseen-until-you-look-at-it.md)
(option A, inside tmux only). Amends ADR 0214 decision 1 (a second way
for a turn in a mux to be seen) and its consequences (the gap a timer
refresh closed only by luck). Uses the visibility watcher of
[ADR 0108](0108-handoff-refreshes-run-in-the-background.md)'s
hand-off and the tmux server match of
[ADR 0205](0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md)'s
amendment. Adds no program, subcommand, socket, signal, write path, key,
effect, theme role, or glyph (ADR 0117, ADR 0143, ADR 0153, ADR 0154):
the watcher's existing `tmux display-message` read asks for one more
format variable.

## Context

ADR 0214 reads a ready turn in a mux as seen once the mux's last attach
time is at or after the turn's end, or a snapshot loaded after it lists
a client attached. tmux moves `#{session_last_attached}` when a client
attaches or switches to a session, not while one stays, so a turn that
ends while the operator watches it in an attached client is seen only if
a refresh happens to list that client while it is still there. An
operator who switches back to the TUI sooner saw the turn and still finds
it bold (unseen) until their next attach.

At the moment the client comes back to the TUI's session, no refresh can
list it on the mux it watched: it is already gone from there. What is
missing is the session it came from. tmux has it as
`#{client_last_session}`, the session the client was on before the one it
is on now, and the TUI already asks tmux about its own pane every 250 ms
(`display-message -p -t <pane>`, the watcher that refreshes the screen
when the operator comes back).

A probe against a private tmux server showed what the value does:

| client | `#{client_last_session}` read for the TUI's pane |
| --- | --- |
| on the TUI's session, never switched | empty |
| TUI, then `mux`, then back on the TUI | `mux` |
| TUI, `mux`, `other`, back on the TUI | `other` |
| back on the TUI, then a window change inside the TUI's session | still `mux` |

The last row matters: the value is where the client was before it came to
the TUI's session, however long ago, so it says nothing about a window
change.

## Decision

1. **The watcher's read gains the client's last session.** The poll's
   format becomes
   `#{session_attached}:#{window_active}:#{pane_active}:#{client_last_session}`.
   Nothing else about the poll changes: the same call, the same
   interval, `display-message` already sanctioned for tmux. Outside tmux
   there is no watcher and nothing changes.

2. **A return is read when the client comes back to the TUI's
   session.** The watcher notes the poll that finds a client attached to
   the TUI's session after a poll that found none, and reads the client's
   last session there, where it is the session the client came from. A
   client that comes straight back to the TUI's window is the same poll
   as the pane turning visible (ADR 0108); one that comes back to another
   window of the TUI's session is credited then, and the later window
   change credits nothing. The return is dated by the last poll that found
   the client away, so a turn that ended between that poll and the return
   (less than a poll before it) is never credited. A poll that fails says
   nothing; a client that has not switched sessions (an empty value)
   names none.

3. **The turns it watched read seen.** A ready turn of an agent whose mux
   is the session the client came back from, on the TUI's own tmux
   server, and that ended at or before the return's time, is seen. The
   server is matched as the live-state binding matches one (the socket
   path `$TMUX` names), and the session by its tmux name. The TUI keeps
   the latest return from each session in the same in-memory memory as
   the turns it found seen (ADR 0214 decision 3); a turn judged seen this
   way joins that memory like any other, so the anti-flicker property
   holds and a turn that ends later is unseen until it is looked at.

4. **In memory, no effect.** The rows are judged again in place when a
   return arrives, as when a snapshot is put on screen: no effect is
   emitted, nothing joins the effect lifecycle table, and nothing is
   written. A new TUI starts without returns.

## Consequences

- A turn that ends while the operator watches it in an attached client
  reads seen the moment they are back on the TUI's session, with no
  attach and no timer refresh. A turn that ends after the client left
  for another session, or on a session the client was not last on
  (`mux`, then `other`, then the TUI reads `other`), stays unseen: the
  failure side is the unseen one.
- **Known limit: two clients on the TUI's own session.** tmux expands a
  client format for a session against the client of it that was most
  recently active, so with two clients on the TUI's session the value may
  belong to the other one. The watcher also sees a client on the session
  the whole time, so it finds no return at all while another client stays
  there; the gap of ADR 0214 remains for that operator. A wrong credit
  needs both clients to have left the session together and returned
  apart.
- A client that leaves and returns within one poll (250 ms), or whose
  mux was renamed meanwhile, is not credited.
- A TUI outside tmux sees no change: the hand-off runs `attach-session`
  in the foreground, the client is gone when the TUI resumes, and ADR
  0214's rule stays as it was.
- The capability manifest names this ADR in tmux's `adr` list; its
  entries are the same.

## Alternatives Considered

- **Read at the pane turning visible, as the decision first worded it.**
  The same poll in the usual case, but a window change inside the TUI's
  session reads a last session from an earlier trip and would credit
  turns that ended after the client had left it (the last row above).
- **Poll `list-clients` while the TUI is away.** It reads the client's
  session as it moves, but it is a new tmux subcommand, and outside tmux
  it would run through the whole foreground attach. Out of scope.
- **Infer the session from the refresh at the return.** The client
  stayed on the mux from its last attach until the next later attach
  time of another session. Unsound: mux, then X, then Z, then X, then the
  TUI leaves X's time after the client left the mux, and a turn that
  ended on the mux after that would read seen.
- **Keep the gap.** ADR 0214's consequences as they were.

## Open Questions Answered

- *Does the credit outlive a refresh that lists nothing of the sort?*
  Yes: the turn joins the seen-turns memory like any other.
- *Does it persist?* No; nothing is written, and a new TUI starts
  without returns.
