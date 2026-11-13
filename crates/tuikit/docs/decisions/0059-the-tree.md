# 0059: The tree keeps its state by identity and draws lines its host lays out

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/tree_view.md](../controls/tree_view.md) |

## Decision

- **Rows through a trait.** `TreeRow` gives a row's identity (`Id: Ord +
  Clone`), its depth, and whether it has children; a row's children are the
  deeper rows that follow it. A host implements it on its own row type;
  `tree::Row<Id, R>` is a plain shape for others.
- **State apart from the rows.** `TreeState<Id>` keeps the selected identity
  and the position it was placed at, the expanded set, and the window the
  last frame drew; every method takes the rows, which it never stores. The
  cursor is on the copy of its identity nearest the kept position. Moves
  stop at the ends and page by the kept window; folds act on the selected
  row; `reveal` opens a row's ancestors.
- **One rebuild routine.** `begin_rebuild` remembers the cursor's identity
  and position; `finish_rebuild` puts it back by the first rule that
  applies: the same identity (the nearest copy), else the same position,
  else the host's first-load hint, else the first row. The host may change
  the folds between the halves.
- **The pane.** `TreeView` draws a pane frame with a title, an optional
  header line, an optional footer banner, and an empty-state placeholder.
  The rows the window shows (only those) come from the host's producer, a
  closure from a row and its `RowCtx` to a line; the pending and ghost
  looks apply, then the cursor gutter before every row and the highlight
  over it. `RowCtx` tells the producer the width after the gutter, the cells
  the scrollbar took, and whether this is the one row the cursor is on (of
  two copies, the one the cursor stands on). `render` returns the window it
  drew.
- **What the host knows, worked out before the frame.** A row's state (ghost,
  pending, a mark the host puts on it) is data passed beside the rows, so
  the producer stays pure. Page keys carry no height.
- Rows laid out from column schemas come from `TreeColumns` (record
  [0060](0060-column-schemas.md)), with the gutter and indent as the lead.

## Rationale

The host had eight tree views. A state that owned its rows fought the
host's rebuilds and its many readers of the rows; the old retention put the
cursor on the first copy of an identity, against its own intent, so a
refresh moved it.

## Consequences

- The tree is house rule 1's last list: it marks its cursor row in the
  gutter like every other list.

## Alternatives considered

- **A generic row struct the host adopts.** Every use of the host's row
  would change for no change in meaning; the trait keeps it.
- **A state that owns its rows.** Every reader would go through it.
- **A closure for row states called while drawing.** Lookups would stay in
  the draw.
- **The page height in the message.** A second source for the window's size.
- **First-copy retention.** It moves the cursor on refresh in lists that show
  an identity twice.
- **A gutter column in every schema.** Every producer would fill it; the
  tree owns the cursor, so it draws the gutter once.
- **Marking every copy of the selected identity.** Two markers say nothing
  about which one the cursor is on.

## Origin

- host ADR 0182, "The tree keeps its state by identity, and draws lines its
  host lays out": decisions 1 to 6.
- host ADR 0186, "The tree marks its cursor row in a gutter before every
  row": decisions 1 to 3.
- host ADR 0195, "A tree row keeps its layout when its list overflows":
  decision 2 (the producer is told the scrollbar's cells).
