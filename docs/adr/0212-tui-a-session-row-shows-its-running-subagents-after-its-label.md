# ADR 0212: A Session Row Shows Its Running Subagents After Its Label

## Status

Accepted. Carries out the row half of the operator's decision 8 in
section 7.1 of the live-state plan (`docs/plans/live-state/README.md`):
"a dim `+N` chip after the row label while any subagent is active ... no
nested rows". The detail pane's listing is the detail-pane story's.
Amends [ADR 0209](0209-tui-session-and-mux-rows-show-live-state-in-a-one-cell-column-after-the-age.md)
decision 1 (live state now also draws a note after a session's label)
and the session row's give-way order of
[ADR 0183](0183-tui-rows-come-from-column-schemas-and-one-rule-decides-what-gives-way.md).
Adds no theme role (ADR 0153) and no glyph (ADR 0154).

## Context

Every harness reports how many subagents a session runs (Claude Code's
and Codex's subagents, opencode's child sessions, Pi's subagent tool
calls), and the row models carry the count beside the state.
`conspectus table` shows it as `+N`, and hides it once a session reads
`exited` or `unknown`, whatever count was last recorded. A wait raised by
a subagent already reads `!` on its root session. The TUI showed none of
it. The decision left open: how big N may get, which rows show the chip,
where exactly it sits, and when it gives way.

## Decision

1. **`+N` right after the label.** Session rows, in every view that
   lists them (Sessions, the agents nested under a mux or a PR, Search),
   gain a note column right after the label and before the `was in`
   note, the project, the hit count, and the preview. A session with no
   label shows it right after its link cell. It is an inert
   `StatusChip`, as the `was in` and `N hits` notes are.

2. **Capped at three cells.** `+1` to `+9`, then `+9+` for ten or more,
   so the column is bounded. The count itself stays in the detail pane
   and the CLI.

3. **One rule with the CLI.** The chip shows while the agent reports one
   or more subagents and its state is neither `exited` nor `unknown`
   (`LiveState::shows_subagents`, which `conspectus table` now reads
   too). A session without live state shows none.

4. **Session rows only.** A mux row shows none (its agents' rows do, and
   on a mux row `+N` already counts the harness chips that did not fit);
   Pinned and History rows show none.

5. **It yields to the label.** On a narrow row the session's notes give
   way in the order preview, `was in` note, project, subagent chip, hit
   count, and then the label narrows and leaves: the label keeps its
   width while the chip can leave, and a content search's hit count,
   the reason a row is in a search's results, outlasts it.

## Consequences

- In the showcase, ambiguous candidate B reads `+3` and the pinned
  session `+1` (its wait is its subagent's question).
- A row with the chip loses its preview four cells sooner; the pinned
  session's preview no longer fits at 170 columns in the Sessions view.
- `+N` means two things by row kind: subagents on a session row, more
  harness chips on a mux row.
- The help page's Agent state legend gains a `+1` row.

## Alternatives Considered

- **A row per subagent.** A parallel burst would multiply rows; the
  decision rules it out.
- **After the live mark, in the lined-up cells.** It would move every
  label on every row by the chip's width, for a note most rows lack.
- **Show the exact count (`+12`).** Unbounded, and the exact number is a
  detail-pane fact; ten or more reads as a burst either way.
- **Keep the chip for an exited or stale session.** It would claim
  subagents run where the session's own state says it can't tell.
- **Let the chip outlast the hit count.** In a search's results the hit
  count is why the row is there.

## Open Questions Answered

- *Is the chip dimmed further when the session's state is quiet?* No: it
  already recedes, and the quiet mark beside it says so.
