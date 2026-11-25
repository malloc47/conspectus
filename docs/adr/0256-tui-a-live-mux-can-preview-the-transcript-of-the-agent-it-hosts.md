# ADR 0256: A Live Mux Can Preview The Transcript Of The Agent It Hosts

## Status

Accepted. Amends [ADR 0112](0112-node-specific-previews.md) (decision 1, the
`MuxSession` row of the table) for a live mux, on the operator's opt-in.
Extends the `[tui]` keys of [ADR 0012](0012-config-file-layout.md) with
`mux_preview`. No new key, write path, or process. Amended by ADR 0258:
the mux that hosts this TUI previews its agent's transcript too (decision 7).

## Context

A live mux previews its pane: `tmux capture-pane`, laid out by the wrap mode
(ADR 0106). When an agent runs in that pane, the capture is the agent's own
interface drawn inside the Preview, with its borders, status line, prompt box,
and colors, next to Conspectus's. The two interfaces compete, and the agent's
reads as clutter against the rest of the screen. The operator (2026-11-25)
asked for an opt-in, through configuration, to the Preview a mux already gets
when it is not live (ADR 0253): the session view, which is the transcript of
the session it hosts, in Conspectus's own styling. They wanted it to update as
often as the pane's Preview does.

`[tui] session_preview` (ADR 0112 §4) is the same kind of choice for the other
direction: it lets a session in tmux show its pane. Nothing chooses for a mux.

Neither Preview refreshes on a timer. A pane is captured, and a transcript
read, when the cursor comes to rest on its row (ADR 0237), on `r`, and when a
tmux hand-off returns (which marks the pane and the transcripts of the
sessions in it for reload). `RunConfig::mux_preview_interval` is parsed and
unused; a timer for both is open work (backlog `CSP-185`).

## Decision

1. **`[tui] mux_preview` chooses what a live mux previews.** `pane` (the
   default, today's behavior) or `transcript`. It is configuration only: no
   controls-overlay section, no entry in the state file, no key.
2. **With `transcript`, a live mux that hosts an agent session previews that
   session's transcript.** The Preview is the transcript the native viewer
   renders for a session's own row: bottom-anchored on its newest turns,
   scrolled by `J` and `K`, hidden by `--no-live-preview`.
3. **The session is the one `v` opens on the mux.** That is the
   resolver-preferred active `linked_to_mux` link, so the Preview and `v`
   always name the same session.
4. **It applies wherever a live mux previews its pane.** The Mux view's rows,
   the Pinned view's row for a pin on a live mux, a mux node drilled into, and
   a Related row's neighbor all resolve to the same source. (The History view
   lists only what is gone, so a live mux has no row there.)
5. **The pane stays where there is no transcript to show.** A live mux with no
   live agent session linked to it, and one whose preferred session's harness
   the viewer cannot read, preview their panes as before. A transcript file
   that has gone says so, as the session's own row does, rather than falling
   back.
6. **The transcript is not routed through the session's own rule.** The
   session's row under `session_preview = "pane"` would show the mux's pane,
   which is the screen this option avoids. The mux previews the transcript
   whatever `session_preview` says; that setting still decides a session row.
7. **Nothing else moves.** A stopped mux previews the session it hosted last
   (ADR 0253). The mux that hosts this TUI previews nothing, as the pane
   would show the TUI itself, whichever value is set. A session row, a pin
   row on a session, and every other node kind resolve as before.
8. **The cadence is the pane's.** The transcript is a source like any other,
   so it loads when the cursor rests on its row, on `r`, and when a hand-off
   returns, which is when the pane loads; no timer is added for it. A test
   holds the two to the same sequence of loads.

## Consequences

- An operator who opts in reads the conversation in Conspectus's colors and
  layout, and scrolls it as they scroll any transcript.
- The transcript can lag what the pane shows: a prompt waiting on a
  permission, a tool still running. That is why the pane is the default (ADR
  0112 §4); an operator who needs it attaches, or leaves the key at `pane`.
- A mux whose active pane is a shell beside the agent shows the agent's
  conversation rather than the shell, since the session is found by link, not
  by pane.
- A mux and the session it hosts share one cached transcript. Moving the
  cursor between their rows does not reload it, the source being the same; `r`
  does.
- Where several sessions link to a mux, only the one `v` opens is shown.

## Alternatives Considered

- **Reuse `session_preview`.** It answers what a session in tmux shows, with
  the pane as its non-default value; extending it to muxes would tie two
  separate choices together and give one key two meanings.
- **A section in the controls overlay, remembered in the state file.** It is
  how `session_preview` and `preview_wrap` work, and it makes the choice
  reachable without editing a file. The operator asked for configuration, and
  the overlay is one more place for the setting to be out of step with the file;
  it can be added the way ADR 0112 §4 did.
- **A timer for the transcript alone.** It would make the transcript fresher
  than the pane it replaces, and the operator asked for the same cadence. A
  timer for both belongs to `CSP-185`.
- **Fall back to the pane when the transcript file is missing.** A session that
  has just started has no transcript yet, and the pane is then the useful
  view; but the same fallback would hide a path or permission problem the
  operator should see, which a session row reports plainly.
- **Show the transcript for the mux that hosts this TUI.** The recursion that
  hides its pane does not apply to a transcript, so it could be shown. Left for
  the operator to ask for: it changes a documented rule (ADR 0112, ADR 0253)
  for a case that opting in does not need.
- **Call the value `session`.** "Session" names both a mux session and an
  agent session in this codebase, and the setting is about telling them apart.

## Open Questions Answered

- **Which session, when a mux hosts several?** The one `v` opens on it.
- **Does a harness without a transcript parser lose its preview?** No, it
  keeps the pane.
- **Does the setting change what a session row shows?** No; `session_preview`
  decides that.
