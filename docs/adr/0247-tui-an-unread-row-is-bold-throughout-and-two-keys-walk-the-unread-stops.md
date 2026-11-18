# ADR 0247: An Unread Row Is Bold Throughout, And Two Keys Walk The Unread Stops

## Status

Accepted. Amends
[ADR 0245](0245-tui-an-agent-that-stops-reads-unread-until-you-read-it-and-reads-persist.md)
(the weight is on the row, not only on the mark; the stops can be walked)
and follows
[ADR 0216](0216-tui-one-key-jumps-to-the-agent-that-has-waited-longest.md)
(the walk reuses the jump's placing of an agent on a row). The keys are
bindings of the scoped keymap
([ADR 0191](0191-tui-the-status-bar-names-the-keys-of-the-scope-that-has-them.md)),
so no new write path or effect is added.

## Context

ADR 0245 draws an unread stop bold on its one-cell live mark. The operator
rebuilt against it and did not see the cue working: a single bold glyph in
a small colored cell is nearly invisible in many terminal fonts, and the
cursor row is bold anyway. They asked (2026-11-17) for the row to carry the
emphasis, and for keys that go to the previous and the next unread mux. Their
first idea, `Shift-j` and `Shift-k`, is taken: `J` and `K` scroll the right
pane's preview, and `[` and `]` switch views.

## Decision

1. **An unread row is bold in its words.** A session row whose live stop
   is unread draws its key, its label (a title; an alias is bold already),
   and its last-message preview bold as well as its mark; a mux row draws
   its name and its preview bold. Colors, glyphs, widths, and alignment do
   not change, so a read and an unread row differ only in weight. The
   harness chip, the age, and the link glyph keep the style they had: the
   chip and age belong to the framework's cells, and the words are enough
   to read a row by. Rows of other kinds (group headers, Pinned, History,
   Search, PRs) are as they were; Pinned and History labels are bold
   already, and the word in their status cell is bold while unread (0245).
   The cursor row looks as it did, and the detail pane's `state` still
   says `unread` or `read` in words.
2. **Two keys walk the unread stops.** `}` goes to the next agent with an
   unread stop and `{` to the previous one, from either pane and every
   view. The order is the oldest stop first, the way `W` orders waits, and
   the walk wraps. A press from a row that stands for no unread agent goes
   to the first (`}`) or the last (`{`). The status line says `2 of 4
   unread, since 1h ago`, or why the cursor stayed: nothing is unread, no
   other agent is, or the filters hide them. They are the braces because `]`
   and `[` already switch views, in the same pairing of a key and its
   shifted brace, and `J` and `K` stay the preview scroll.
3. **The walk stays in the view while it can.** `W` goes to the row in the
   view on screen, else switches to the view that lists the agent. The
   unread walk prefers the rows of the view on screen: it takes the next
   agent in its order that this view has a row for (in Mux, its mux's
   row, which is why an unread mux is a stop of the walk), and only when no
   other unread agent has a row here does it do what `W` does and switch
   (Sessions; Mux for an agent in a pane no session matches), with
   Backspace back. A session with no mux is therefore not an "unread mux":
   pressing `}` in the Mux view does not leave it for one.
4. **The binding follows the stops.** Both bindings are available only
   while some agent has an unread stop, so the status bar names `}` next
   unread then and only then, the help page lists both from their rows,
   and the legend's Unread entry names both keys from the keymap. No hint
   is written by hand.

## Consequences

- An unread row reads unread from across the room; a long list of agents
  that stopped is a list of bold rows.
- The cursor row keeps hiding the weight, so the Unread legend and the
  detail pane remain the places that say so in words.
- The walk and `W` differ in one rule (decision 3), documented on the help
  page and in `docs/tui-columns.md`; `W` is unchanged.
- The age and the chip stay as they were, so an unread row is not wholly
  bold; adding a weight option to the framework's age cell was not worth a
  public API for a cue the words already carry.

## Alternatives Considered

- **Bold the whole row, chips and ages too.** Needs a weight on the
  framework's age and chip cells, a public addition, for little the words
  do not already show.
- **A new glyph or a leading marker for unread.** Moves every column and
  changes the grid every view shares (ADR 0235-style churn) for a cue
  weight already gives.
- **`Shift-j` and `Shift-k`.** Taken by the preview scroll, which the
  operator also uses; rebinding it would trade one navigation for another.
- **`n` and `N`, or `u` and `U`.** `n` and `N` are new and launch a mux, and
  `u` is undo; the braces cost nothing already bound.
- **Make `W` stay in the view too.** Changes an accepted decision (0216)
  nobody asked to change.
- **Walk newest first.** `W` goes longest first and the operator's habit is
  one order for both walks.

## Open Questions Answered

- *Is it only muxes?* The operator said "unread mux"; in the Mux view the
  walk visits mux rows (and the agent rows nested under them), and in the
  Sessions view session rows, because an agent is what is unread.
