# 0054: A palette is a query over results the host ranks

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/palette.md](../controls/palette.md) |

## Decision

- **Rows are the host's.** A row implements `PaletteRow`: `cells` lays it
  out for the width it is told (after the cursor gutter, and after the
  scrollbar's column when the results overflow), knowing whether the cursor
  is on it, the query, and the theme; `haystack` is what the built-in filter
  matches. The palette draws every row through the shared cursor look, cut
  in cells. `Item<T>` is the built-in row. A host can instead lay rows out
  with a closure and its own theme (`draw_with`), and route keys without the
  overlay trait (`handle_key`).
- **Ranking is the host's.** A palette over known candidates takes a ranker
  (the query and the rows in, the indices to list out, best first) and
  reranks on every edit; `every_word` is the built-in one. A palette fed by
  a provider lists the rows it is handed.
- **One cursor rule.** An edit, a scope switch, and an option change put the
  cursor on the first result, the best one; new rows keep it where it was
  while that is still a result. The host may move it (`select`). Lists
  wrap; pages stop.
- **One table, the host's verbs.** `PALETTE_KEYS` binds `Ctrl-C`, the arrows
  and their chords, `PgUp`/`PgDn`, `Enter`, and `Esc`; every other key
  edits the query, so `q`, `j`, and `k` type. A palette with a scope or
  options includes the table and binds them under its own verbs. A host
  with actions of its own implements `PaletteAction` and resolves its own
  first. The footer comes from whichever table the palette resolves with.
- **One look.** The `PALETTE` size, which fills its extent so the frame
  keeps its size as results narrow: the query after a `/` prompt in
  `accent`, with its caret while the results have the keys; a status row
  the host sets (with `Progress` words for a running scan); the results,
  or a note for an empty list; the footer. The title is the name, the scope
  strips, and the count. The results scroll by the follow rule with the
  shared scrollbar; `reserve_scrollbar` keeps the bar's column so rows keep
  one width as results stream in.
- **Scopes, options, and a debounce.** The first scope strip is the
  palette's (its key cycles it); further strips are the host's. The options
  pane lists named on/off settings in the results' place. A debounced
  palette marks every change; the host polls `take_due(now, force)` and
  runs the search itself.

## Rationale

The host's filter pickers, quick select, and content search were one shape
with three implementations, and a command palette would have been a fourth.
Leaving ranking and rows to the host keeps the palette generic; owning the
query, cursor, keys, and look makes every instance behave alike.

## Consequences

- A row of several cells is laid out by a column schema (record
  [0060](0060-column-schemas.md)).

## Alternatives considered

- **Options as a step pushed over the palette.** A second frame with a
  confirm, where options should apply as they change.
- **A window re-followed from the top every frame.** It drags the list when
  the cursor moves back up.
- **Keeping the cursor where it was after an edit.** It leaves the cursor on
  a row that has nothing to do with what was typed; a host that needs that
  rule restores it with `select`.
- **Scope and options keys in the standard table.** Every palette would show
  them, and the verbs are the host's words.
- **Host keys in a second table** the host resolves first. Its footer would
  be stitched from two tables.
- **No reserved column.** Columns would move by a cell as a list starts to
  overflow.
- **Progress composed inside the palette.** It would tie the host's nouns
  into palette state; a formatter keeps the row the host's.

## Origin

- host ADR 0174, "Palettes are one control, a query over results the host
  ranks": decisions 1 to 8.
- host ADR 0175, "Search is two palettes, and a hit is one column plan":
  decisions 2, 4, 5, and 6 (rows the host draws, the reserved scrollbar
  column, a cursor the host can move, and the progress words).
