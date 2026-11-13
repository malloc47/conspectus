# 0060: Rows come from column schemas, and one policy decides what gives way

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/columns.md](../controls/columns.md) |

## Decision

- **A schema.** A `ColumnSchema<K>` is a row's columns in order, keyed by the
  host's key type. Each column has a width rule: `Fixed(n)`; `Cap(n)`, the
  widest cell measured under its key, at most `n` (each cell counted at most
  its cap, and cut to it); steps by the room left or by the whole row's
  width; `Hug`, the cell's own width, after which the row flows; a share of
  the whole row, or of what the other columns leave, within a minimum and a
  maximum; or the `Fill`, which wants at least a minimum and may hand its
  cells to a named column when it does not show. A column also has an edge,
  a gap after it (two cells unless told), and how it gives way: it drops at
  a priority, narrows at a priority to a floor, or leaves below a room.
  Options cover a ragged column and one that absorbs the row's lead.
- **Typed cells.** A `Cell` knows its width and cuts one way: text at its
  end, in its middle (paths, identifiers), or clipped, with a tail kept
  whole; styled spans and chips; a chip list with a count of the rest; the
  age cell; a snippet cut around its match.
- **One measure pass** goes over every row of a list once a frame (rows a
  fold hides included), keeping each key's widest cell and the least lead
  seen. Schemas that share a key share the column.
- **One narrow-width policy.** In its room a row (1) leaves out columns
  below their room; (2) gives every column its width, the fill its minimum
  (7 cells unless told); (3) while too wide, gives way in ascending
  priority, the later of two at one priority first and a narrowing before
  a leaving, narrowing no more than needed and taking back what a later
  leaving freed; (4) gives the fill the rest, shown only with its minimum
  or its whole text; (5) still too wide, cuts at its edge with `…`. The
  columns before the first hugging one line up: they give way by the widths
  measured over the whole list, so every row in one room loses the same
  ones. A room can count cells the row's cells do not fit in (a scrollbar's
  column), so width rules count the whole width while cells fit the rest.
- Gaps are blank, in the default style. Every row of several cells the
  framework draws, in a tree or a palette, lays out by a schema.

## Rationale

The host had ten row layouts, each with its own pre-passes and cutoffs, and
a palette had a column plan that repeated the policy without measuring. One
schema and one policy make every row give way the same way at every width
and keep lined-up columns lined up.

## Consequences

- A host's other table output may keep an allocator of its own (one that
  shrinks without dropping, over plain text); the two share the text
  module's measure and cut, not the policy.

## Alternatives considered

- **Growing the palette's column plan.** It measured nothing, could not drop
  its fill, and had no keyed columns; it was retired instead.
- **Laying every column out row by row.** A long cell on one row would drop
  that row's column and leave the others', and the columns after it would
  stop lining up.
- **A fill minimum of 8.** A round number, but it hides content the old rule
  showed.
- **Each row in its own room when a column absorbs the lead.** A deeper row
  would reach its floor first and the columns after it would move.
- **Reserving the scrollbar's column in every list.** Every row would lose
  its last cell while it fits.
- **A width rule that calls the host.** An escape hatch the policy cannot
  reason about; the share of the rest and the fill that hands on its cells
  are the general forms.

## Origin

- host ADR 0183, "Rows come from column schemas, and one rule decides what
  gives way": decisions 1 to 5 and 8.
- host ADR 0184, "More row kinds come from schemas, and group headers carry
  two cells": decision 4 (the capped column, the lead rule, and drawing part
  of a row in another look).
- host ADR 0185, "A search hit is one column schema in the modal and the
  search view": decisions 1 and 2 (shares and row steps, and the plan
  retired).
- host ADR 0186, "The tree marks its cursor row in a gutter before every
  row": decision 4 (the least lead).
- host ADR 0195, "A tree row keeps its layout when its list overflows":
  decision 1 (the room).
- host ADR 0196, "Quick select's entity rows come from a column schema":
  decisions 1 and 2 (a share of the rest, and a fill that hands its cells
  on).
