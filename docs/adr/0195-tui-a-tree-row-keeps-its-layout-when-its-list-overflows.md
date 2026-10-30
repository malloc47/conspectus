# ADR 0195: A Tree Row Keeps Its Layout When Its List Overflows

## Status

Accepted. Amends
[ADR 0183](0183-tui-rows-come-from-column-schemas-and-one-rule-decides-what-gives-way.md)
(the narrow-width policy) and
[ADR 0182](0182-tui-the-tree-keeps-its-state-by-identity-and-draws-lines-its-host-lays-out.md)
(what the tree tells its producer): a row is laid out in a room whose width
rules can count cells its cells do not fit in, and the tree tells its
producer the cells the scrollbar took. Keeps
[ADR 0175](0175-tui-search-is-two-palettes-and-a-hit-is-one-column-plan.md)
decision 4 (the search palettes reserve their scrollbar's column) and
[ADR 0156](0156-tui-scroll-window-follow-rule-and-viewports-by-value.md)
decision 5 (the gutter is carved only on overflow). Carried out by CSP-786.

## Context

`TreeView` carves the scrollbar's gutter from the pane only while its rows
overflow it (ADR 0156, "fade on fit"), and told its producer the width
left. Every row was then laid out one cell narrower once the list grew
past the pane, and the width rules that read the width moved with it: a
column stepped by the room left for it (the mux name), stepped by the
whole row or a share of it (the Search view's hit name and tag, ADR 0185),
or left below a room. At 160 columns the Search view's rows are 78 cells
while they fit and 77 once they overflow, which took a hit's tag from its
20-cell step to its 14-cell one (`turn 46 · mes…`) and moved the snippet
as results streamed past the pane.

The search palettes do not move this way because they reserve the
scrollbar's column (ADR 0175 decision 4): their rows always end a cell
before the frame, as they always did. The left pane's rows have always
taken its whole width while they fit.

## Decision

1. **A room can count cells beside the row.** `tuikit::columns::Room` is
   the cells a row is laid out in: the cells its width rules count
   (`whole`) and the cells its cells fit in (`cells`), apart when
   something takes the end of the row some of the time
   (`Room::new(width).beside(1)`). Steps 1 and 2 of the narrow-width
   policy (the rooms a column leaves below, the steps by the room left,
   the steps by the row, the shares) count the whole width; steps 3 to 5
   (giving way, the fill, the cut) fit the cells. The fill takes what is
   left of the cells, so it gives the bar its cell while it is above its
   minimum; where it is at its minimum the policy's next column gives way,
   as it would in any narrower room. `ColumnSchema::layout`, `draw`,
   `draw_split`, and `draw_then` take `impl Into<Room>`, and a `usize` is
   a room of that many cells, so every other list is unchanged.
2. **The tree tells its producer the bar's cells.** `RowCtx::scrollbar` is
   the cells the scrollbar's gutter took from the row: 1 while the rows
   overflow and the gutter is carved, 0 otherwise. `width` is still the
   cells the row may take. `TreeColumns` lays every row out in
   `Room::new(width + scrollbar).beside(scrollbar)`, so a tree whose rows
   come from schemas keeps every column where it is while the list fits.
3. **The tree does not reserve the column.** The left pane's rows keep the
   last cell while they fit, as they always have; only the frames where a
   list overflows a pane tall enough for the bar change, and there the
   stepped and shared columns read as they do while the list fits.

## Consequences

- At 160 columns a Search view hit keeps its 20-cell tag as results
  stream past the pane; its snippet ends a cell earlier beside the bar.
  Where the tag's step was reached only by the bar's cell (an 80-column
  stacked pane), the overflowing rows now read as fitting ones: the tag
  and name take their wider steps and a pane line's command, the first
  cell to give way, leaves, as it does while the list fits.
- A frame whose rows fit, or whose width puts no step, share, or room on
  the bar's cell, is unchanged: before, the rows were laid out a cell
  narrower with the rules counting that width; now the rules count the
  whole width and the fill gives the cell, which is the same row unless a
  rule's threshold lies on that cell.
- A producer that lays its rows out by hand can read `RowCtx::scrollbar`
  the same way; one that ignores it keeps today's behavior.
- The palette and the tree keep two answers to one question: a palette
  reserves the column (its rows always ended before the frame), a tree
  lends it from the fill (its rows always took the pane). Both keep a
  row's columns still while a list grows.

## Alternatives Considered

- **The tree reserves the scrollbar's column**, as the search palettes do
  (rows always end a cell before the border). One rule for both, and no
  change to the column schema, but every row of every view loses its last
  cell while it fits, the mux name's steps move by a cell of pane width
  again, and at 160 columns the Search view's hit tag would keep its
  14-cell step for good; every documented frame would change.
- **Cut the row laid out at the whole width.** Lay the row out as though
  the bar were not there and cut its last cell with `…`. It keeps the
  columns still but marks a cut on rows that had room to give (a
  right-aligned age would read `5…`), against the policy's one cut.
- **Leave it.** The tag and the snippet move by up to six cells when a
  list crosses the pane's height, mid-scan in the Search view.

## Open Questions Answered

- *Does the tree reserve the scrollbar's column?* No: its rows take the
  pane while they fit, and a row of a list that overflows is laid out at
  the pane's whole inner width, its fill giving the bar its cell.
- *Which width do the stepped and shared columns see?* The pane's whole
  inner width after the cursor gutter, overflowing or not.
