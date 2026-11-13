# ADR 0209: Session And Mux Rows Show Live State In A One-Cell Column After The Age

## Status

Accepted. Carries out the operator's decisions 1, 7, 10, and 12 in
section 7.1 of the live-state plan (`docs/plans/live-state/README.md`;
the options are in `tui-options.md` there). Amends
[ADR 0145](0145-state-glyphs-and-words-in-the-left-panel.md): live state
joins the state glyphs (a cell of its own, six marks) and the state
words (four words that take a running session's link word's place).
Amends [ADR 0183](0183-tui-rows-come-from-column-schemas-and-one-rule-decides-what-gives-way.md)
decision 6: the session and mux schemas gain a lined-up column after the
age that never gives way. Reads the data of
[ADR 0203](0203-live-state-is-a-neutral-observation-with-a-closed-state-set.md)
and [ADR 0205](0205-live-state-is-a-snapshot-side-table-bound-to-sessions-and-muxes.md)
as the row models already carry it; adds no theme role (ADR 0153) and one
vetted glyph (ADR 0154). Amended by
[ADR 0214](0214-tui-a-finished-turn-reads-unseen-until-you-look-at-it.md):
a ready mark is bold while its finished turn is unseen. Amended by
[ADR 0215](0215-tui-rows-float-and-filter-by-live-state-only-when-the-tree-is-built.md):
the live state that floats and filters rows is read when the tree is
built; refills still change the cells in place.

## Context

Since the live-state data layer landed, every session row carries its
agent's effective state (state, detail, confidence, since, subagents),
every mux row its agents' counts, and every Pinned and History row the
value of the entity it stands for, kept current between snapshots by the
TUI's overlay. Nothing drew it. Seeing at a glance which agent waits on
the operator, which finished, and which is still at work is the point of
the feature.

The constraints: views of one kind show state as one-cell glyphs and
views of mixed kinds as words (ADR 0145); no state may hide behind an
empty cell or depend on color alone; the common `●◐○` set collides with
the session kind, draft PR, and open PR glyphs; the Braille spinner
already means "refresh pending"; `?` already means "can't be checked" and
`×` is the shared failure mark.

## Decision

1. **A cell of its own, right after the age.** Session rows (in every view
   that lists them: Sessions, the agents nested under a mux or a PR, and
   Search) and mux rows (running and stopped) gain a one-cell column
   between the age and the link glyph. It is always filled. The session
   kind glyph `●` and the link glyph `⊸`/`∘` keep their meanings.

2. **Six marks.**

   | Mark | Session state | Style (theme token) |
   |---|---|---|
   | `!` | `waiting` | warning |
   | `»` | `working`, `compacting`, `starting` | info |
   | `›` | `ready` | success |
   | `×` | `error` (the shared failure mark) | error |
   | `?` | `unknown`, stale (the can't-tell mark) | muted |
   | `·` | no evidence, or `exited` | muted |

   A quiet state (`working`, `compacting`, or `starting` past its source's
   quiet window) draws the same mark with the muted modifier added. The
   colors are the framework's status tokens, which `[tui.theme]` already
   sets; no host role is added. `»` is registered as a vetted glyph; `!`
   and `?` are ASCII, and `×`, `›`, and `·` are framework glyphs. The
   character of `›` is the breadcrumb separator's, a reviewed second use:
   one draws only in the live cell, the other only between a
   breadcrumb's hops.

3. **A mux shows its agents' mark.** A mux has no state of its own; its
   cell shows the state of its counts that ranks first by the model's
   default precedence (waiting, error, ready, working, then compacting,
   starting, unknown, exited), `·` when nothing is counted. It is never
   dimmed, because the counts do not say how fresh each agent's evidence
   is; the agents' own rows, nested under a mux with more than one, are.

4. **Words in Pinned and History.** A running session's row (a session pin,
   or a session nested under a pin) whose agent is waiting, working
   (compacting, starting), or ready, or whose turn failed, prints
   `waiting`, `working`, `ready`, or `error` in its state-word column in
   place of `in mux` or `resumable`, in its mark's style. Unknown, stale,
   exited, and no evidence keep the link word. Whether the session runs in
   a mux stays in the detail pane. A mux pin keeps `live`; History lists no
   sessions.

5. **No animation.** The marks are static; the spinner keeps meaning a
   refresh is pending. While a row awaits a hand-off's refresh, the spinner
   takes the link or mux-state cell (and in Pinned and History the word)
   as before, and the live mark keeps its value, dimmed with the row: the
   TUI reads live state from its store on its own, so that refresh does
   not decide it, and a row shows one spinner.

6. **It never gives way.** The live column is a lined-up column with no
   priority, like the link glyph: on a narrow row the key, then the chip,
   then the age leave, and the live mark and link glyph stay. It costs
   three cells with its gap, so the label, project, and preview after
   them leave three cells sooner, and a mux name narrows three cells
   sooner.

7. **The renderer reads the row models.** One mapping
   (`tui::rows::LiveSign`) turns a session's cell, a mux's counts, or a
   mixed row's value into the mark, its word, and its style; the left
   pane's cells, the Pinned words, and the help legend all read it. No row
   builder computes live state, so a store change refills the cells in
   place as before.

## Consequences

- Every session and mux row is three cells denser. At 170 columns the
  frames look as the operator's mockup; at 50 a session row in the
  Sessions view keeps its key, chip, age, mark, and link but loses the
  label it used to show cut to eight cells.
- `?`, `×`, `›`, and `·` each carry a second meaning by context, listed in
  the reference document's glyph table.
- In the default theme `info` and `success` are both green, so working and
  ready differ by shape alone; an operator who wants them apart sets
  `info` (which also colors Info messages).
- The showcase world records live state for every mark, so its fixture
  and the reference frames show them, and the README screenshots will
  when they are next rendered.
- The help page gains an "Agent state" legend that names the command that
  turns a harness's hooks on.

## Alternatives Considered

- **Replace the session kind glyph with the mark** (option B). No extra
  width, but a session's identity glyph would change with its state, and
  nested and Pinned rows would need the same treatment.
- **A state word after the link cell** (option C). Self-explaining, but
  nine cells, and it breaks the rule that views of one kind show glyphs.
- **Animate `working`.** The spinner already means a pending refresh, and
  a static mark is cheaper to draw and works with reduced motion.
- **Let the live cell give way after the age.** It would keep the label
  three cells longer at the narrowest widths, but drop the one thing the
  feature exists to show.
- **Host theme roles for each state** (`live_working`, …). They would let
  working and ready differ in color by default, but the decision names
  the status tokens, which already fit and are configurable.
- **Spin the live cell too during a refresh.** Two spinners on one row, for
  a value the refresh does not decide.
- **Compose the Pinned words** (`in mux · waiting`). Wider than the
  column, and the link fact is in the detail pane.

## Open Questions Answered

- *Does a mux pin in Pinned show its agents' word?* Not yet: it keeps
  `live`, which the filled pin marker already says. A rolled-up word there
  belongs with the mux rollup work.
- *Does a resumable session with live state change its word?* Yes: an
  agent that reports a state is running, so `resumable` would mislead; the
  rule is the link word, either of them.
