# ADR 0201: A Hand-Off Freeze Ends Per Mux, And At Once When The Mux Is Still There

## Status

Accepted. Amends ADR 0108 (decision 2, one mark for every returned mux, and
decision 4, the first result after the latest hand-off settles it, with the
re-probe its refresh ran) and ADR 0132 (decision 3, the block ends with the
refresh).

## Context

After a return from tmux, the mux's row and the rows of its linked sessions
are dimmed with a spinner and refuse actions until a refresh lands
(ADRs 0108, 0132), in case the mux exited or changed while the operator was
in it. The operator hit two problems with that.

1. **It is slow when the mux still exists**, which is the usual case. The
   refresh after a hand-off probed again while the set of running muxes
   equalled the one from before the hand-off, up to twice, 150 ms apart. A
   detach that leaves the mux running is exactly that unchanged case, so the
   common path paid three full discoveries, each nudging the daemon's `mux`
   and `harness` classes (or a local discovery without a daemon), plus
   300 ms, before the row was released. The retries exist for the opposite
   case: a session that just ended, or a server just killed, can still be
   listed for a moment.
2. **A freeze waited on later hand-offs.** The mark was one set, and only
   the refresh after the newest hand-off settled it. Outside tmux,
   `tmux attach-session` blocks the UI thread for as long as the operator
   stays in a second mux; the first mux's refresh result waits undrained,
   and the return from the second mux moved the settle point past it before
   the drain read it. The first mux then spun until the second mux's
   refresh landed, however long the operator had been away. Inside tmux
   `switch-client` returns at once, and every return of the TUI's pane into
   view and every new attach moved the settle point forward again.

What tmux answers was measured on tmux 3.6a against a private server, with a
client attached through a pseudo-terminal. Right after the client returns,
`display-message -p -t <session id> '#{session_id} #{session_created}'`
answers:

- after a detach: the same session id and creation time;
- after the session's last pane exits, or after `kill-session`: empty
  fields, with exit status 0;
- after `kill-server`: an error, `no server running`.

An empty target resolves to some other session. So the exit status alone
proves nothing; the answer has to be compared with what the screen showed.

## Decision

1. **Each frozen mux settles on its own.** The runtime records, for each
   frozen mux, the generation of the refresh it started after that mux's
   latest return. Any applied result at or after that generation releases
   the mux and the rows of its linked sessions. A later hand-off never moves
   an earlier mux's point: outside tmux, the first mux's waiting result
   releases it on the first drain after the return from the second;
   inside tmux, each mux is released by its own refresh, and the pane coming
   back into view, which freezes nothing, holds nothing. Results still apply
   in spawn order (ADR 0108 decision 4). The hand-off itself, which an
   entity to follow waits for, still ends with the newest hand-off refresh.
2. **An existence check releases a mux that is still there.** On a return,
   the reducer keeps the mux's incarnation from the snapshot the screen
   showed before the attach: backend, socket, tmux session id
   (`backend_session_id`) and creation time (`created_epoch`). The refresh's
   worker asks the mux's server once, through `exec::command`:
   `tmux [-L <socket>] display-message -p -t <session id>
   '#{session_id} #{session_created}'`, read-only, on a thread beside the
   discovery so it never delays the refresh. When both fields match, the
   mux and its sessions' rows are released at once, and the refresh updates
   everything behind them. When they don't, or the call fails, one re-check
   follows about 150 ms later, and only a matching re-check releases. A new
   session that took the name has another id, and a restarted server that
   reused the id has another creation time, so neither matches.
3. **Only what the check couldn't confirm is probed again.** The refresh
   probes again (at most twice, 150 ms apart, as before) only while its
   listing still shows a returned mux the check didn't confirm or couldn't
   ask about. A refresh for the pane coming back into view keeps the old
   rule (probe again while the running muxes are exactly the ones the screen
   showed), since it froze nothing that a check could confirm.
4. **What isn't checked waits for the refresh.** A mux with no recorded
   incarnation (an older snapshot, or a backend without session ids such as
   zellij) and a mux a launch ran in before the attach (a rebuild) stay
   frozen until the refresh lands, and are probed again while listed: the
   session surviving says nothing about the agent session a rebuild
   relaunched in it. A mux the screen didn't show running (one the hand-off
   created) has nothing to wait out.
5. **No new capability.** `display-message` is already a `read` subcommand
   of `tmux` in `docs/capabilities.toml`; the entry cites this ADR for the
   new call (ADR 0117).

## Consequences

- A detach that leaves the mux running releases its rows after one tmux
  call and the next frame, instead of three discoveries and 300 ms.
  Toggling between muxes is never blocked by an earlier mux's freeze.
- A mux that exited on detach stays frozen until a refresh shows it gone,
  including when the listing still shows it for a moment after the exit.
- A released mux's rows show pre-hand-off values until the refresh lands,
  as rows outside the mark always could (ADR 0108). The check confirms the
  session, not what runs in it: a mux whose session survives with a dead
  pane (`remain-on-exit`) is released before the refresh shows its new
  state, and so is a mux in which the operator ended one agent session and
  started another.
- If some tmux answered for a session for a moment after it ended, the
  check would release that mux early and its refresh wouldn't probe again
  for it. The measurements above found no such window; the re-probe stays
  for the listing.
- Each return costs one more tmux call, two when the mux is gone, and one
  short-lived thread beside the refresh.
- The loop's discovery and existence check are injectable, so the live loop
  is tested end to end with scripted answers: a mux still there, gone, gone
  while still listed, confirmed only by the re-check, and two muxes in a row
  outside and inside tmux.

## Alternatives Considered

- **Check the active pane's PID**, as the operator suggested. The snapshot
  already has `active_pane_pid`, and `kill(pid, 0)` needs no subprocess.
  But it is a signal: the manifest allows signal 0 only in the `act`
  context (ADR 0093), so the read-only TUI would need a new allowance.
  PIDs are reused, so a live PID doesn't prove the same incarnation. And a
  pane's process answers a different question: it can exit while its
  session lives (`remain-on-exit`), or be replaced (`respawn-pane`) while
  the session is the same. The freeze asks whether the session outlived the
  hand-off. The probe is a trait, so a PID check could replace the tmux
  query without touching the settle logic.
- **Keep one settle point and shorten the retries.** Fewer re-probes would
  shorten the common path a little but still wait for at least one full
  discovery, weaken the exit-race cover, and leave a later hand-off holding
  an earlier mux.
- **Ask by name, or by exit status alone** (`has-session -t =<name>`). A new
  session that reused the name would release the freeze on the old one.
- **Do nothing.** The freeze stays correct but slow, and a mux can stay
  frozen for as long as the operator is in another one.

## Open Questions Answered

- **Session query or pane PID?** The session query; the PID check is the
  recorded alternative.
- **Does the re-check delay a release?** Only when the first answer doesn't
  confirm the mux.
- **What does the pane coming back into view freeze or release?** Nothing.
  Its refresh still ends the hand-off when it is the newest.
- **Does the call need a new manifest entry?** No new subcommand: the
  `tmux` entry already lists `display-message`, and now cites this ADR.
