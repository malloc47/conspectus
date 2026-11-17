# ADR 0245: An Agent That Stops Reads Unread Until You Read It, And Reads Persist

## Status

Accepted. Amends
[ADR 0214](0214-tui-a-finished-turn-reads-unseen-until-you-look-at-it.md)
(which stops count, the words, and "nothing persisted") and
[ADR 0231](0231-tui-a-turn-watched-in-an-attached-client-reads-seen-when-the-client-comes-back.md)
(a return counts for every stop, not only a finished turn). The marks go
in the TUI state file that [ADR 0143](0143-mutation-envelope-with-history-pins-and-clone.md)
already counts among the category 2 observations, so no write path is
added; the gestures reuse the persist effect and the key tables
([ADR 0191](0191-tui-the-status-bar-names-the-keys-of-the-scope-that-has-them.md)).

## Context

ADR 0214 marks a `ready` turn bold until the operator has seen it, from
evidence the TUI already has (a client attached to the mux after the turn
ended, or the cursor resting on a session that has no mux) and keeps what
it found in memory. The operator asked (2026-11-16) for the same cue when
an agent goes from working to waiting, whether or not it asks for input,
and for three things 0214 left out: a way to mark one agent read, a way to
clear every unread mark, and a mark that outlives the TUI. Reattaching to
an agent already reads it.

"Waiting" in the operator's words covers every way an agent stops and
hands the next move to them: it asks for a permission, a question, or a
plan (`waiting`), it finishes a turn (`ready`), or its turn fails
(`error`). The operator chose all three: waiting, ready, and error.

Memory only has a cost 0214 accepted when the only reader was a turn
found seen by looking: a restart marked every finished turn of a session
with no mux unread again. With a gesture that marks a stop read on
purpose, losing it at the next start would undo what the operator did.

## Decision

1. **Every stop can read unread.** An agent whose live cell is `waiting`,
   `ready`, or in `error` has stopped, and its stop is unread until it has
   been read. Working, compacting, starting, exited, and unknown are never
   unread. The judgment is 0214's, unchanged, applied to the time the
   shown state began (`since`): the stop is read when a tmux client
   attached to the agent's mux at or after that second (0214 decision 1),
   when the operator's client came back to the TUI's session from that mux
   after it (0231), when the cursor rested on the session's row for a
   second while it has no mux (0214 decision 2), or when the operator
   marked it read (decision 5). A stop found read stays read until the
   agent stops again, whatever a later refresh shows (0214 decision 3). An
   agent that has never run, and a state read from a poll that cannot tell
   a finished turn from an interrupted one, read as they did: a `ready`
   with no evidence is unread.
2. **The weight is the same, on more marks.** An unread `!`, `›`, or `×`
   (and Pinned's `waiting`, `ready`, and `error`) draws bold in its own
   color; a read one draws plain. A mux's mark is bold while any agent
   counted in the state it shows is unread, which the model keeps as a
   count per state. The detail pane's `state` field ends in `unread` or
   `read` for every stop, a mux's `agents` says `1 waiting (1 unread)`,
   and the glossary and the help legend say the same.
3. **The words are unread and read.** The operator reads them; the model
   keeps its older names (`unseen`, `SeenTurns`) in code, noted where they
   are defined, so the history of ADR 0214 stays readable.
4. **Reads persist in the TUI's state file.** `tui-state.json` gains
   `read_marks`: for each agent session, its node id in text form and when
   the latest stop read began (Unix epoch milliseconds). They are loaded at
   start and written through the existing persist path when a mark moves
   (a tick that found one, a quit, and each gesture), skipped when
   nothing changed, and merged on every write with what is in the file, the
   later stop winning, so two TUIs do not undo each other. The file keeps
   the 2,000 newest; one bad entry, or marks of another shape, cost only
   themselves, and a run with `--no-resume-view` reads them and writes
   nothing. Only agent sessions have marks: an agent in a pane that no
   session matches has no identity that outlives its pane, so its reads stay
   in memory. A mark older than a session's latest stop is no mark of that
   stop, which is why a later stop reads unread.
5. **Two gestures, in the TUI (backlog CSP-893).** `c` marks the selected
   row's agents read: a session's, a mux's counted agents', or a pinned
   entity's. `C` clears every unread mark. Both are key-table bindings (so
   the footer, status hints, and help list them), both are entries of the
   row action menu, and both persist through the effect that already
   writes the state file. Neither offers undo: reading is not a change to
   take back, and a later stop is unread again.

## Consequences

- A wait the operator has not looked at is as visible as a finished turn.
  A quick run of permission prompts leaves its trail of bold marks until
  the operator attaches, rests on, or marks them.
- A restart no longer revives the unread marks the operator cleared. A
  session with no mux that was read in an earlier run stays read.
- The state file grows by one short line per session read, bounded at
  2,000, and gains writes it had not: the first time the cursor rests on a
  session, and each attach seen.
- A new session of a harness the operator launches is `ready` with no
  detail and unread until attached or rested on, as before.
- Stops of agents in panes no session matches, such as a bare Codex pane,
  are unread again after a restart.

## Alternatives Considered

- **A separate `read-marks.json`.** One more file and one more write
  path for the same observation; the state file already has the atomic,
  skip-on-unchanged writer.
- **Persist marks for panes too.** A pane's identity (the tmux server's
  socket, session, and pane id) does not survive the server, so a mark
  would never match again.
- **Persist only the gestures, not the evidence.** A turn seen by
  attaching would read unread again next start, which is the flicker 0214
  worked to avoid.
- **Expire marks by age.** The bound by count does it without a clock.
- **Rename the code to unread.** A wide change to code and tests for no
  behavior; the words an operator reads are the ones that changed.
- **Mark `working` unread when it ends in nothing.** A state the agent
  leaves by itself is not a stop; only the three that hand the move to the
  operator are.

## Open Questions Answered

- *Does a `ready` that no turn preceded count?* Yes, as in 0214: the
  evidence cannot tell it from a finished turn, and attaching or resting
  reads it.
- *What does clearing all do to a stop that has not happened yet?* Nothing:
  it marks the stops that exist; the next stop is unread.
