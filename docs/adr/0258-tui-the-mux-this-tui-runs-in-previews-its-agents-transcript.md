# ADR 0258: The Mux This TUI Runs In Previews Its Agent's Transcript

## Status

Accepted. Amends [ADR 0256](0256-tui-a-live-mux-can-preview-the-transcript-of-the-agent-it-hosts.md)
(decision 7, the mux that hosts this TUI) and the exception that
[ADR 0112](0112-node-specific-previews.md) and
[ADR 0253](0253-tui-a-stopped-mux-previews-the-session-it-hosted-last.md)
make for it. No new key, setting, or write path.

## Context

The mux a TUI runs in has always previewed nothing: its row said `no mux
target for this row`, since capturing its pane would show Conspectus inside
itself. ADR 0256 let a live mux preview the transcript of the agent it hosts
and kept that exception, because it was a rule the operator had not asked to
change. The operator (2026-11-25) runs an agent in the same tmux session as
Conspectus, and with `mux_preview = "transcript"` set, every mux showed its
agent except that one.

The reason for the exception does not reach a transcript. It is read from
the agent's session file, not captured from the pane the TUI draws in.

## Decision

1. **Under `[tui] mux_preview = "transcript"`, the mux that hosts this TUI
   previews the transcript of the session `v` opens on it.** It does so in
   every place a live mux previews: the Mux view's row, the Pinned view's row
   for a pin on it, a mux drilled into, and a Related row's neighbor.
2. **Its pane is still never previewed or captured**, whatever the setting.
   Attaching to it is still refused. This changes what the Preview reads,
   not what Conspectus will do to the tmux session it runs in.
3. **Without an agent to show, it previews nothing.** Under `pane`, or with
   no live agent linked to it, or one whose harness the viewer cannot read, the
   row says what it said before.
4. **Nothing else moves.** Every other live mux, a stopped mux (ADR 0253), and
   a session row resolve as ADR 0256 has them.

## Consequences

- An operator who opts in gets the same Preview on every mux, their own
  included, and can read their agent's conversation from the mux row beside it.
- The mux Conspectus runs in can have several panes, and the agent may be in
  one the TUI is not in. The session is found by link, not by pane, so the
  transcript is right either way.
- The placeholder for a hosting mux with nothing to show is still `no mux
  target for this row`, which describes it only loosely.

## Alternatives Considered

- **Capture the pane when the TUI is in a window other than the one tmux
  has active.** A capture takes the session's active pane, which Conspectus
  cannot know to be another window's; getting it wrong draws the TUI inside
  itself, which is what the exception prevents.
- **Say why the row is empty.** `no mux target for this row` could read `this
  is the tmux session Conspectus is running in`, which is more accurate. It
  changes a string shared with other rows and is a separate change.
- **Show the transcript whichever value `mux_preview` has.** Under `pane` the
  operator has asked for panes; a transcript on one mux would be a surprise
  among the rest.

## Open Questions Answered

- **Does this make the mux attachable?** No. Attaching to the session the TUI
  runs in is still refused.
