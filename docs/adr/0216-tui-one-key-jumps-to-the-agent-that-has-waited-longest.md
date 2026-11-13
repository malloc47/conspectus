# ADR 0216: One Key Jumps To The Agent That Has Waited Longest

## Status

Accepted. Carries out the operator's decision 5 in section 7.1 of the
live-state plan (`docs/plans/live-state/README.md`; the options are in
section 4 of `tui-options.md` there): a key that, from any view, moves to
the next waiting session, longest-waiting first, waiting only, wrapping,
chosen from the free keys of the scoped keymap, with its hint generated.
Adds one binding to the keymap of
[ADR 0192](0192-tui-the-main-screens-keys-are-one-scoped-keymap-and-the-help-page-is-generated-from-it.md);
lands on rows as reveal and quick select do
([ADR 0198](0198-tui-quick-select-and-reveal-promise-only-a-view-that-lists-the-entity.md));
reads the waiting marks of
[ADR 0209](0209-tui-session-and-mux-rows-show-live-state-in-a-one-cell-column-after-the-age.md)
and counts what the header's `! N` counts
([ADR 0211](0211-tui-the-header-counts-every-agent-waiting-on-you.md)).
Adds no effect, no theme role (ADR 0153), no glyph (ADR 0154), and no
write path (ADR 0143).

## Context

The header says how many agents wait on the operator anywhere and the
group headers say where, but reaching one means unfolding groups and
scrolling, or switching views when the one on screen does not list the
session. The tools surveyed answer this with one key that goes to the next
agent that needs you, after which attaching is the gesture the program
already has.

The decision left the key to be chosen from the free keys, preferring
`!`, the waiting mark. It left open which row stands for a session in a
view that lists it in another shape (the Mux view shows a mux's only agent
on the mux's row; Pinned shows a pin), what counts as the selection being
one of them, what happens to agents in panes no session matches (the
header counts them), what to say when nothing can be reached, and whether
the jump is an effect.

## Decision

1. **`W`, "waiting".** `!` already opens the Messages log, whose unseen
   chip and failure messages name it, so the waiting mark cannot be the
   key. `W` is free in both panes' scopes and in the program's, reads as
   the word, and binds as typed like every character of the keymap
   (`w` is the worktree menu). The binding `waiting.next` is the
   program's (it works with either pane focused), lists under Navigation
   on the help page, and names itself in the status bar as `W next
   waiting` only while some agent waits (`available_when`), ranked with
   the selection's own keys.

2. **What waits, in what order.** The agents are those whose mark reads
   `!` in the values the rows were last filled with: every session with
   live state and every agent in a pane no session matches, the set the
   header's `! N` counts, whatever the view lists. They are ordered by
   when their wait began (the cell's `since`), the longest first, ties by
   entity so the order is total. Ready, unseen, and error turns are not
   in it.

3. **Where each one is.** In the current view a session is at its own
   row, the first that stands for it (a session row, a pin on it, or a
   session listed under a pin); in the Mux view, a session with no row
   of its own (the only agent of its mux, or one no link attaches) is at
   its mux's row, the mux its live state counts on. An agent in a pane
   no session matches is at its mux's row. When the current view has no
   such row, the agent is in the view a reveal of it lands in: Sessions
   for a session, Mux for a pane's agent, when that view lists it under
   its own filter. An agent no view lists is skipped.

4. **Next, from where the cursor is.** When the selected row is where a
   waiting agent is, the key goes to the next agent in the order whose
   place is another row, wrapping; otherwise to the first. In the current
   view the cursor lands on that row's copy with the groups above it
   expanded (`TreeState::select_at`). Elsewhere the key switches to the
   other view as a reveal does, so `Backspace` returns.

5. **It says where it went, in the status line.** A jump says which of
   how many waiting it reached and since when (`2 of 4 waiting, since 1h
   ago`), and, when it switched views, which and why (`switched to
   Sessions: Pinned doesn't list it`). With nothing waiting, with only
   the selected agent waiting, or with every waiting agent hidden by the
   views' filters, the cursor stays and the status line says so as a
   warning, the severity of every other key that has nothing to act on.

6. **Navigation, not an effect.** Moving the cursor and switching views
   are view state; a switch persists the view as `SwitchView` does. No
   effect is added, so the lifecycle table is unchanged. Attaching to the
   session it reached is the existing gesture (`Enter` or `a`).

## Consequences

- With the showcase's waiting sessions in three groups, `W` from the top
  visits the Ungrouped session first (it has waited two hours), then
  `project`'s pinned one, then the codex fork under its parent in
  `repo-a`, unfolding each group on the way, then the newest wait.
- The order follows the marks at the moment the key is pressed, so an
  agent that started waiting since the last press joins the cycle where
  its wait's start puts it.
- In the Mux view the key can land on a mux row with several waiting
  agents; pressing again moves on to the next row, not to each agent.
- A session hidden from the Sessions view by its filter is skipped, and
  a status line that says how many wait counts it; `F` there brings it
  back.

## Alternatives Considered

- **`!`, the waiting mark.** The decision's first choice, but it opens
  the Messages log, which ADR 0105 gave it and the unseen-messages chip
  names; moving that key would retrain the operator for a less frequent
  overlay. Rebinding the log was out of this decision's scope.
- **`@` or `` ` ``.** Free, but neither says "waiting", and the shifted
  letter mirrors `w`, `n`/`N`, `s`/`S` in the same table.
- **Most recent wait first.** The decision chose the longest; the
  operator who has waited longest is the one most likely blocked.
- **Always switch to Sessions.** Simpler, but it would leave the Mux view
  for an agent its own mux row already shows, and a Pinned operator for
  a pin already on screen.
- **Leave out agents in panes no session matches.** They are counted in
  the header's `! N`, and a new session that waits on its first prompt
  often has no transcript yet; leaving them out would make the count and
  the key disagree.
- **A toast for every jump.** The status line is where the program says
  what a key did, and a toast would cover the rows the jump just showed.

## Open Questions Answered

- *Does the key work with the right pane focused?* Yes: it is the
  program's binding, it moves the left tree's cursor, and the right pane
  follows the selection as it always does; focus stays where it was.
- *Does the jump clear a filter to reach a hidden session?* No: it skips
  it; changing what the operator filtered is their call.
