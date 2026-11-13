# ADR 0186: The Tree Marks Its Cursor Row In A Gutter Before Every Row

## Status

Accepted. Carries out house rule 1 of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md) for the last
list without its marker, the left pane's tree, as
[ADR 0159](0159-tui-lists-show-their-cursor-one-way.md) decision 6 left it
to do once the tree's rows came from a column schema
([ADR 0182](0182-tui-the-tree-keeps-its-state-by-identity-and-draws-lines-its-host-lays-out.md)
decision 7,
[ADR 0183](0183-tui-rows-come-from-column-schemas-and-one-rule-decides-what-gives-way.md),
[ADR 0184](0184-tui-pr-pinned-and-history-rows-come-from-schemas-and-headers-carry-two-cells.md),
[ADR 0185](0185-tui-a-search-hit-is-one-column-schema-in-the-modal-and-the-search-view.md)).
Amends ADR 0159 decision 6 (the tree draws the gutter), ADR 0182 (one
copy of a listed-twice identity is the cursor row, where every copy was
highlighted), and ADR 0183 (a column that absorbs the lead does not absorb
a lead every row shares).

## Context

Every menu, form, picker, search list, checklist, Messages, and the right
pane drew `marks::POINTER` in a 2-cell gutter before its cursor row; the
left pane's tree showed its cursor row by its highlight alone (reversed
and bold with focus, bold without). With `selection_active` and
`selection_inactive` set to nothing, or a terminal that draws no reverse
video, the tree had no cursor cue. Its rows began with the disclosure and
kind-glyph cells `docs/tui-columns.md` documents, so the gutter waited
until every row kind was laid out from a schema; since ADR 0185 all six
views are.

Facts established while adding it:

- The schema counts a row's lead in a share of the row and in steps by
  the row (ADR 0185), and the search modal passes its cursor gutter as
  the lead. A tree whose lead left out its gutter would see a row two
  cells narrower than the modal's at one width, and the hit's turn column
  (22, 20, or 14 cells by the row) would step at other pane widths.
- A column that absorbs the lead (a group header's label, a Pinned or
  History row's label) gave way as a row with no lead would in the whole
  width. With the gutter in every row's lead it would absorb the gutter
  too, and its floor would drop by two cells at every depth: a top-level
  Pinned label could narrow to 6 cells, not 8.
- The mux name steps by the cells left for it (18, 20 from 39, 24 from
  73, 30 from 105); the gutter takes two of them, so the name would step
  down 4 or 6 cells at pane widths where nothing else changed. No other
  left-pane rule counts the cells left: floors, fill minimums, shares, and
  row steps do not move.
- `TreeView` highlighted every copy of the selected identity (the Mux and
  PRs views can list a row twice). With a marker, two marked rows would
  contradict "one cursor row".

## Decision

1. **The tree draws the gutter.** `TreeView` draws every row through
   `selection::row`: the producer's line gets the pending and ghost looks,
   then the gutter before it (`►` and a space on the cursor row, focused
   or not, two blank cells on every other row) and the cursor's highlight
   over both. The producer is told the width after the gutter
   (`RowCtx.width`), so nothing it lays out moves as the cursor does.
   `RowCtx::row(line)` is that drawing for a host that draws one row on
   its own; it replaces `apply_looks`. The header line, the footer banner,
   and the placeholder are not rows and have no gutter.

2. **One cursor row.** `RowCtx.selected` is true for the one row the
   cursor is on: of two copies of an identity, the copy the cursor stands
   on, which the window already followed.

3. **The gutter is part of the schema's lead.** `TreeColumns` measures
   and draws each row with the gutter and the indent as its lead
   (`TreeColumns::lead`; `indent` is the indent alone), as the modal
   passes its gutter, so a share and row steps count the whole row and a
   hit takes the modal's cells at one width.

4. **A lead every row shares is not absorbed.** The measure pass keeps
   the least lead it saw (`Measure::least_lead`). A schema whose lined-up
   columns include one that absorbs the lead gives way as the shallowest
   measured row would in the whole width, and only a row's own lead (its
   indent) comes off that column. Without a gutter the least lead is 0,
   as every tree has top-level rows, so no existing layout changes.

5. **Thresholds move by the gutter and no more.** The mux name's steps
   become 18, 20 from 37, 24 from 71, and 30 from 103 cells left, so the
   name steps at the pane widths it stepped at. No other threshold
   changes.

6. **Where the two cells come from.** At every width the row's fill (a
   preview, snippet, content, title, or directory) gives them up first.
   Where the fill is already at its minimum, the next cell in the row's
   documented order gives way: a Pinned or History row's content leaves
   below its 7 cells, a hit or pane row's age leaves, a flat project or
   a Pinned label narrows.

## Consequences

- Visible, in every view and at every width: the cursor row starts with
  `►`, focused or not, and every row's cells sit two cells right of where
  they were. A row listed twice is marked and highlighted once. The
  Search view's header line and the Pinned banner stay where they were.
- Narrow panes lose two cells of what the row's order gives up first; a
  frame where a cell was within two cells of leaving now drops it.
- `docs/tui-columns.md` documents the gutter as the first cell of every
  row and its frames are regenerated; the mux name's steps there read 37,
  71, and 103.
- Tests: the framework's tree draws the gutter and marks one row (one of
  two copies), tells the producer the width after it, keeps the text
  after it still as the cursor moves, and counts it in a share; the
  column schema keeps a shared lead out of an absorbing column's floor.
  The host's consistency suite checks the tree in every view at every
  cursor position, focused and not: one marked row, highlighted, no other
  row reversed, the marker alone finding it with the selection pair
  emptied, and the text after the gutter unchanged as the cursor walks
  every row. The Pinned, History, PR, and menu snapshots show the
  marker and the shift.

## Alternatives Considered

- **A gutter column in every schema.** The control page's seam suggested
  it. Every schema would declare the same column and every producer fill
  it; the tree owns the cursor, so it draws the cursor's gutter once.
- **The gutter outside the schema's lead.** The producer's width would
  shrink and the schema would not know; shares and row steps would see a
  row two cells narrower than the modal's of the same width, against ADR
  0185, and the turn column would change by up to 6 cells at some widths.
- **Absorbing the gutter.** Simpler, but every absorbing column's floor
  would fall by two cells, and a top-level Pinned label could narrow to
  6 cells.
- **Keeping the mux name's steps.** The name would step down 4 or 6
  cells at widths two cells past the old ones; moving the steps by the
  gutter keeps every pane width's name width.
- **Marking every copy of the selected identity.** Two `►` rows in one
  list say nothing about which the cursor is on.
- **Indenting the Search header line by the gutter.** The header names
  the listing, as the pane's title does; it is not a row the cursor
  visits.

## Open Questions Answered

- *Does the gutter come from a column of the schema?* No: from
  `TreeView`, through `selection::row`, with the schema told it is part
  of each row's lead.
- *Do the narrow-width rules change?* Only the mux name's steps, by the
  gutter's two cells; the drop orders, floors, and minimums are as
  documented.
