# ADR 0234: A Pinned Mux Reads The Word Of Its Agents' Mark

## Status

Accepted. Carries out the operator's decision of 2026-11-13 on the
question [ADR 0209](0209-tui-session-and-mux-rows-show-live-state-in-a-one-cell-column-after-the-age.md)
left open. Amends ADR 0209 decision 4 (a mux pin keeps `live`) and the
state-word table of
[ADR 0145](0145-state-glyphs-and-words-in-the-left-panel.md) it extended.
Uses the mark rule of ADR 0209 decision 3 and
[ADR 0233](0233-tui-a-mux-mark-recedes-when-every-agent-in-its-state-is-quiet.md).
Adds no theme role (ADR 0153), no glyph (ADR 0154), no key, no effect,
and no write (ADR 0143).

## Context

In Pinned, a running session pin reads `waiting`, `working`, `ready`, or
`error` in place of `in mux` or `resumable` (ADR 0209 decision 4). A mux
pin kept `live`, which the filled pin marker already says, although the
mux's row in the Mux view draws a mark for the state of its agents. ADR
0209 left a rolled-up word for a mux pin to the mux rollup work; that work
(the mux rows' marks and the group headers' counts) did not cover Pinned,
so the same mux read `»` in one view and `live` in another.

## Decision

1. **A pinned mux reads its mark's word.** In Pinned (and History), a row
   standing for a running mux whose agents' mark is `!`, `×`, `›`, or `»`
   prints `waiting`, `error`, `ready`, or `working` in place of `live`,
   by the precedence of its row's mark in the Mux view: waiting, then
   error, then ready, then working (compacting and starting read
   `working`, as for a session). It is the same sign
   (`LiveSign::of_mux`) that draws the mark, so the word and the mark
   cannot disagree.

2. **In the mark's style.** The word takes its mark's color, dimmed when
   the mux's mark is dimmed (ADR 0233) and bold while a ready mux's
   mark is bold (ADR 0214), as a session's word does.

3. **Nothing to say keeps `live`.** A mux with no agent that has live
   state, or whose agents read can't-tell (`?`) or only exited (`·`),
   keeps `live`, as a session with no evidence keeps its link word.
   Stopped, removed, missing, planned, and unknown rows keep their words.

4. **The consistency suite holds it.** One mux shows one state in its
   Mux-view mark and its Pinned word: the suite derives the expected word
   of every mux pin from the model's counts and compares it with what is
   drawn, and a test sets a mux's agents through each state and compares
   the mark with the word.

## Consequences

- A pinned mux says what its agents are doing in the Pinned view, with no
  need to open the Mux view. The pin marker still says it is running.
- `live` now means a running mux (or a worktree, repo, branch, workspace,
  or fork) with nothing live to say about it.
- In the showcase, the pinned `project-main` reads `working`, dimmed
  because the agents behind it are quiet (ADR 0233).
- The word takes up to seven cells, as a session's does; no column
  changes width.

## Alternatives Considered

- **Keep `live`.** The decision of ADR 0209; the operator chose to change
  it.
- **A count (`2 waiting`).** The words are one word and the column nine
  cells wide; the Mux view and the detail pane carry the counts.
- **Words only for muxes with a waiting agent.** The same mux would read
  `»` and `live` while it works, which is the inconsistency this removes.

## Open Questions Answered

- *Does History say it too?* Only for a row standing for a running mux;
  History lists stopped and removed things, whose words do not change.
- *Does a pinned mux's word follow a stopped mux's mark?* A stopped mux
  reads `stopped`.
