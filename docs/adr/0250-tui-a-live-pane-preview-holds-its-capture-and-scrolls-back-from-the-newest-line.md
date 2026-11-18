# ADR 0250: A Live Pane Preview Holds Its Capture And Scrolls Back From The Newest Line

## Status

Accepted. Amends
[ADR 0106](0106-preview-pane-wrap-modes.md) (the capture is
no longer cut to the zone) and follows
[ADR 0181](0181-tui-rich-text-is-a-framework-feature-and-the-preview-scrolls-in-a-region.md) (the
region that scrolls the Preview). No new key, effect, or write path.

## Context

`J` and `K` scroll the Preview. A transcript opens on its newest turns and
`K` goes back through the rest. A mux's Preview is the pane's capture
(`tmux capture-pane` with the last 100 lines of scrollback), but the
Preview built only the bottom `height` rows of it, so the zone held nothing
above what it showed and `K` did nothing. The operator (2026-11-18) hit it
on a mux row with a live Claude Code pane and expected the Preview to scroll
back as a transcript does.

## Decision

1. **The Preview holds the whole capture, newest row last.** A live pane's
   Preview is every row of the capture laid out in the wrap mode (ADR 0106),
   not the zone's bottom rows.
2. **It scrolls from the newest line.** The region counts from its end
   (`Anchor::Bottom`, as a transcript's does): the Preview opens on the
   pane's newest output, `J` toward it stops there at once, `K` goes back one
   row a press, and the place is kept as a distance from the newest line
   through each refresh of the capture and until the Preview shows something
   else. A pane that prints while the operator is scrolled back moves the
   rows under the same distance; it does not follow the output until
   scrolled back to the newest line.
3. **A header keeps the old shape.** A failure banner, or a Related row's
   evidence, above a capture still leaves the capture the rows under it and
   cuts it to its newest ones: that Preview does not scroll. A content
   search that marks rows of the capture shows the rows around its matches,
   as before.

## Consequences

- `K` works on a live pane. The capture's depth is the 100 scrollback lines
  and the screen; going further back is the pane viewer's job.
- A busy pane drifts under a scrolled-back Preview by the rows it printed;
  `J` back to the newest line follows it again.
- Frames of a short capture are unchanged.

## Alternatives Considered

- **Anchor the place to a row of the capture.** A capture has no stable row
  identity: tmux's lines carry no number, so a row could only be matched by
  its text.
- **Scroll the capture with a header above it.** The header would leave the
  screen at the newest line, where it is read most.
- **Capture more scrollback.** A longer capture is a slower tmux call every
  two seconds; the pane viewer reads up to 5,000 lines on demand.
