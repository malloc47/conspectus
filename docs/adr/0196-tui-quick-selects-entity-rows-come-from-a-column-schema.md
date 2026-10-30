# ADR 0196: Quick Select's Entity Rows Come From A Column Schema

## Status

Accepted. Amends
[ADR 0183](0183-tui-rows-come-from-column-schemas-and-one-rule-decides-what-gives-way.md)
(the column schema's width rules) and
[ADR 0175](0175-tui-search-is-two-palettes-and-a-hit-is-one-column-plan.md)
(whose alternatives left the entity rows on a split of their own). Builds
on [ADR 0185](0185-tui-a-search-hit-is-one-column-schema-in-the-modal-and-the-search-view.md):
every search result is now laid out by the column schema. Carried out by
CSP-787.

## Context

Quick select's wider reach lists entities: a kind glyph, the entity's
label, its context (a directory, a branch and root, a title), and the
outcome word that says what opening it does (`in Sessions view`). The
rows kept a split of their own (`entity_columns`): the word a third of
the row, at most 16 cells, at the right edge; the label 45% of what the
cursor gutter, the glyph, the word, and the gap before it leave, between
16 and 48 cells; the context the rest, unless it would get fewer than 10
cells, when it left and the label took its cells.

ADR 0185 moved the content hits onto `tuikit::columns`, but two of the
entity split's rules were not width rules there: a share of what the other
columns leave (a `Share` is of the whole row), and a fill that, leaving,
hands its cells to a neighbor (a fill that leaves leaves blank cells). The
column schema's page and the palette's page listed them as seams.

## Decision

1. **A share of what is left.** `Column::share_of_rest(key, per, of)`
   (`Width::ShareOfRest`, bounded by `.within(min, max)`) takes `per` of
   every `of` cells of the row's room less every column that shows and is
   neither such a share nor a fill, and less the gaps beside those
   columns: the shares and the fill split what is left, the gaps between
   them included. It is worked out after the other width rules, so a
   stepped column counts the room left before it. It lines up like a
   share, and after a hugging column splits what that row leaves.
2. **A fill that hands its cells on.** `Column::fill(key).gives_to(other)`
   makes a fill that does not show on a row (it dropped below its
   minimum, or no row of the list has a cell for it) hand the cells it
   would have taken to the column under `other`, past that column's own
   rule. A fill that shows takes what is left as before.
3. **The entity rows are a schema.** `entity_columns()` in the search
   rows declares the glyph (2 cells, no gap), the label
   (`share_of_rest(45, 100).within(16, 48)`, narrowing last), the context
   (`fill().at_least(10).drops(0).gives_to(Label)`), and the outcome word
   (`share(1, 3).within(0, 16).right()`), with the palette's cursor gutter
   as the lead. `EntityGrid` measures the entity results once a frame (for
   whether any has a context) and lays each out by the one narrow-width
   policy. The label is painted whole and cut by its cell, so the part of
   a match that shows is picked out.

## Consequences

- At every width from 8 to 260 cells the schema splits an entity row as
  the hand split did (a test holds the two together), so the rows read as
  before; only the pad cells' styles changed (a padded context's and the
  word's pad take their text's receding look), which shows nowhere.
- A list in which no entity has a context gives the label the row up to
  the word, where it padded an empty context column.
- A label cut across its match picks out the match's visible part, where
  it picked out nothing.
- Every search result, a content hit and an entity alike, is one column
  declaration laid out by one policy; the current reach's rows (a label
  and a snippet of what matched) have no grid of columns to declare.

## Alternatives Considered

- **Share of the cells the fixed columns leave, gaps and all.** Simpler to
  state, but the label's 45% would be of two cells fewer, moving it a cell
  at many widths and so changing how the rows read.
- **A fill that gives its cells to whatever comes before it.** No key to
  name, but a fill's neighbor is not always the column that should grow
  (a mark or a glyph sits before many labels); naming the column says it.
- **Keep the hand split.** It works, but it is a second narrow-width
  policy for one kind of search row, which is what the column schema
  replaced everywhere else.

## Open Questions Answered

- *Which columns does a share of what is left count?* All but the shares
  of what is left and the fills, with the gaps beside them.
- *Where do a leaving fill's cells go?* To the column it names, or, naming
  none, nowhere: the row ends earlier, as before.
