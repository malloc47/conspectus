# ADR 0215: Rows Float And Filter By Live State Only When The Tree Is Built

## Status

Accepted. Carries out the operator's decision 4 in section 7.1 of the
live-state plan (`docs/plans/live-state/README.md`; the options are in
section 4 of `tui-options.md` there): a float-waiting-first control, off
by default, on the mechanism of the existing float controls; a State
filter row in Controls and `--state` on `conspectus table`; rows that
never re-sort under the cursor between operator actions; no state
grouping and no Attention view yet. Extends
[ADR 0031](0031-tui-filter-view-switching-and-per-view-state.md) (the per-view
filter and the Controls overlay) with one narrowing dimension and one
ordering toggle. Reads the marks of
[ADR 0209](0209-tui-session-and-mux-rows-show-live-state-in-a-one-cell-column-after-the-age.md),
whose decision 7 ("no row builder computes live state, so a store change
refills the cells in place") this record keeps for every refill and
qualifies for a build. Relies on
[ADR 0182](0182-tui-the-tree-keeps-its-state-by-identity-and-draws-lines-its-host-lays-out.md)
for the cursor across a rebuild. Adds no theme role (ADR 0153), no glyph
(ADR 0154), no key, no effect, and no write path (ADR 0143). Amended (see
the amendment below): the Mux view's `N/M sessions` counts the sessions
a listed mux's mark counts.

## Context

Every session row shows its agent's state and every mux row its agents'
rollup, the group headers and the header line count who waits, and the
TUI binds the live-state store again about once a second, refilling the
marks in place. The operator wants the sessions that need them first and
a way to see only them, but rows that re-sort or vanish under the cursor
as agents change state are disorienting, and the tree keeps its rows
still across folds.

The filters and the float controls live in `RowFilter`: narrowing
predicates every list's builder applies to each session through
`SessionMatchInputs`, and ordering toggles (float sessions with a mux,
float muxes with an attached agent, float PRs with a local branch) that
the builders sort by. The builders run when the tree is built: a new
snapshot, `r`, and a change of view, grouping, sort, or filter. The live
overlay's refill does not run them.

The decision left open which values the filter offers, what a mux
matches, what the chip and the row are called, whether the filter reaches
the command line's other commands and the config, and how the float
combines with the controls it sits beside.

## Decision

1. **One mapping for the marks and the filter.** A session's live state
   reads as one of six values, `waiting`, `ready`, `working`, `error`,
   `unknown`, `none`, the six marks of the live cell (`working` takes
   compacting and starting; `none` is no evidence or exited; `unknown`
   is evidence too old to trust). The neutral filter module
   (`filter::LiveStateKey`) holds the mapping and the TUI's mark is drawn
   from it, so a row the filter keeps for `waiting` is a row drawn with
   `!`. Quiet work is `working`, and a ready turn is `ready` whether or
   not it is unseen.

2. **The filter is a set, like mux state.** `RowFilter.live_state` is a
   set of those values; a session passes when its value is in it. It is a
   narrowing predicate: it hides stopped muxes like the others, counts
   toward `N/M sessions`, and `Clear all filters` (and `F`) clears it.
   A mux passes when an agent attached to it passes, its listed agents
   narrowed to those that do, as for every session predicate, or, when
   no other session predicate is set, when an agent its mark counts
   passes: a session whose preferred mux it is, attached by a resolved
   link or not (an ambiguous one), or an agent in one of its panes that
   no session matches. So a mux drawn with `!` stays under
   `agent:waiting`. Pinned and History ignore it, as they ignore the
   other filters.

3. **Controls: an `agent state` row that steps.** The row sits after
   `mux state` in every view, as its siblings do, and Space steps it
   `any`, `waiting`, `ready`, `working`, `error`, `any`, the values the
   operator named. A set the command line or the config gave
   (`waiting,error`, `none`) reads as its values and steps to `any`. The
   row is named `agent state`, not `state`, beside `mux state` and the
   PRs view's `PR state`. The status bar's filter chips show it as
   `agent:waiting`, between `mux:` and the PR chips, since `state:` is
   already the PR state's.

4. **The command line and the config.** `--state` on `conspectus table`,
   `conspectus tui`, and `conspectus search` takes the six values,
   comma-separated or repeated, as `--mux-state` does, and errors on any
   other with the list; `[tui.views.<view>.filters]` takes
   `state = ["waiting"]`, entries OR-ing together, with a diagnostic for
   an unknown value. The command line reads each session's effective
   state at the moment it prints, with the current clock; a mux row of
   `table mux` matches by decision 2.

5. **Float waiting first, per view, off by default.** `RowFilter`
   gains `float_waiting_top`, an ordering toggle like its siblings. In
   the Sessions view (`Float waiting sessions to top`) a session drawn
   with `!` sorts above the rest of its group, of the flat list, and of
   its parent's forks, each part keeping its order; groups do not move.
   In the Mux view (`Float waiting muxes to top`) a running
   mux drawn with `!` (an agent counted on it waits) sorts above the
   other running muxes; stopped muxes stay below them. Where the
   view's other float is on too, waiting comes first and the other float
   orders within each part. Each view keeps its own setting with its
   filter in `tui-state.json`, as the sibling checkboxes do, and
   `Clear all filters` turns it off. The PRs view has none.

6. **Order and membership are read when the tree is built.** The build
   reads every session's and mux's live state once, from the live
   tables the rows will carry and at the same moment as the rows'
   values (`RowFilter.live`, filled by `build_tree_for_view`, never
   saved), and the builders filter and float by it. The overlay's
   refill (a store rewrite, a process found gone, time moving a state)
   changes the marks, the counts, and the detail pane in place and
   nothing else: a session that stops waiting keeps its place among the
   waiting ones, and one the filter kept stays listed, until the tree is
   next built for an operator action or a snapshot (a Controls change,
   `r`, a view, grouping, or sort change, or the next snapshot). Folding
   does not build the tree. On a build the cursor keeps its row by
   identity, so a row that moves takes the cursor with it, and a row
   that leaves gives the cursor to its position.

7. **No state grouping, no Attention view.** The `Ctrl-G` cycle is
   unchanged and no view is added.

## Consequences

- With the float on, `project`'s pinned session, waiting for an hour,
  rises in the showcase above the two newer sessions at work, below the
  other waiting one.
- In the Mux view, `N/M sessions` counts the sessions attached to the
  listed muxes, as it does under every filter, so a mux listed for an
  agent it counts but no link attaches (the showcase's `ambiguous`)
  adds none to `N`.
- A session whose wait was answered still reads `›` among the floated
  `!` rows, or stays listed under `agent:waiting`, for up to one snapshot
  interval; the mark is right at once and the order catches up at the
  next build.
- The rows say what the operator filtered on, not only what is true now:
  a filtered list can show a row whose mark no longer matches the chip.
- Every builder test that sets no live state reads every session as
  `none`, so the filter is testable without a store.
- The header's `! N` still counts every waiting agent anywhere, so it can
  exceed what `agent:ready` lists, by design.

## Alternatives Considered

- **Re-sort and re-filter on every overlay refill.** The list would
  always agree with the marks, but rows would move under the cursor
  while the operator reads them, which is what the decision rules out.
- **A checklist editor, like `mux state` and `PR state`.** Any set of
  states in one place, but three keys to reach `waiting` where a step is
  one, and the decision named the values as a cycle from `any`. Sets
  stay available from the command line and the config.
- **Offer `unknown`, `none`, and unseen ready in the row.** The six
  values are all accepted from the command line and the config; the row
  keeps to the four the operator acts on. Unseen is not a value
  anywhere: it is the TUI's memory, which changes by looking, and the
  command line has none.
- **Float waiting groups too.** Moving headers would reorder the whole
  tree whenever an agent waits; the decision floats sessions within
  their group.
- **One float control for every view.** The filter, and so each float,
  is per view already; a shared one would float the Mux view when the
  operator asked it of Sessions.

## Open Questions Answered

- *Does a session waiting through a subagent float?* Yes: its root
  session reads `!`, and the float reads the root's mark.
- *Does a mux with one waiting agent among several float?* Yes: its
  mark is `!` by the rollup's precedence.

## Amendment: the Mux view counts the sessions a mux's mark counts (2026-11-13)

Consequence 2 left `N/M sessions` in the Mux view counting only the
sessions attached to the listed muxes, so a mux listed for the agents
its mark counts added none: under `agent:waiting` the showcase's
`ambiguous` mux, whose mark says two agents wait, sat beside `0/11
sessions`. The header and the mark now agree on what the mux holds.
Everything above stands except as follows.

1. **N counts the sessions the listed muxes hold, each once.** Under a
   narrowing filter, each listed mux holds the agents it lists (its one
   agent inline, or those nested under it), and, while the agent state
   is the only session predicate (when decision 2 lists a mux for what
   its mark counts), the sessions its mark counts in a state the filter
   keeps that no link attaches to any mux: a session the resolver can't
   tie to one mux counts on its preferred one. A session attached to a
   mux is held there, so none counts twice. N is read when the tree is
   built, with the order and the rows (decision 6).
2. **Pane agents add nothing.** An agent in a pane no session matches
   keeps its mux listed and counts in the mark and the header's `! N`,
   but it is not one of the M sessions, so it is not in N either.
3. **Other filters are unchanged.** A session predicate other than the
   agent state lists a mux only by its attached agents, so N counts
   those, as before; so does a filter with no live state.

In the showcase, `agent:waiting` lists the `ambiguous` mux and reads
`2/11 sessions`, and `agent:working`, which lists it and `project`
(whose working agent runs in a pane no session matches), reads `2/11`
for the two working sessions `ambiguous` counts.
