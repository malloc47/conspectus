# ADR 0183: Rows Come From Column Schemas, And One Rule Decides What Gives Way

## Status

Accepted. Carries out the `ColumnSchema` part of the data-display row in
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md)'s control
set (the tree lands with lines its host lays out, then moves to a column
schema one kind of row at a time), applying house rules 13 and 14, and
extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the
framework's `columns` module. Builds on
[ADR 0182](0182-tui-the-tree-keeps-its-state-by-identity-and-draws-lines-its-host-lays-out.md):
the tree's producer can be `TreeColumns`, which lays rows out from
schemas, and the pointer gutter still waits for the last kind of row.
Keeps [ADR 0145](0145-state-glyphs-and-words-in-the-left-panel.md)'s
glyphs and words and [ADR 0157](0157-tui-chips-one-width-rule-and-ages-one-ladder.md)'s
chip width and age cell. Leaves [ADR 0020](0020-width-aware-table-rendering.md)'s
CLI table renderer as it is (decision 7). Amended by
[ADR 0184](0184-tui-pr-pinned-and-history-rows-come-from-schemas-and-headers-carry-two-cells.md):
the PR, Pinned, and History rows are laid out from schemas too, a capped
column caps each cell, rows that absorb their lead give way as a row with
no lead would, and a group header's label absorbs the indent instead of
its path, so nested headers line up.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/02-left-pane-tree.md`, sections 4, 6,
and 12) found no declaration of the left pane's columns: seven span
renderers plus inline header code made about ten layouts, three
alignment structs (`GroupAlign`, `PrAlign`, `MixedAlign`) each measured
the tree its own way, about 25 widths were literals in the renderer, and
about 40 two-space gutters were typed inline. Narrow panes had no shared
rule: a mux row dropped cells and ended in `…`, a session row was cut at
the pane's edge with no mark, a Pinned row dropped its chip, and "room
remains" meant more than 2, 4, or 8 cells depending on the row. A stopped
mux was a Pinned-style row that emulated the Mux view's grid
(`MixedGrid::Mux`) so it would line up with the running ones.

ADR 0182 put the tree's state and pane in the framework with a producer
the host writes. The palette already had a plan of columns
(`palette::ColumnPlan`, ADR 0175) that allocates widths per row width
with drop and narrow orders, but it measures nothing: a column cannot be
as wide as the widest cell in a list.

## Decision

1. **`tuikit::columns`.** A `ColumnSchema<K>` is a row's columns in
   order, each a `Column<K>` keyed by the host's key type: a width rule
   (`Fixed(n)`; `Cap(n)`, the widest cell measured under the key, at most
   `n`, none when every cell is blank; `Steps`, a width by the room left
   for the column; `Hug(n)`, the cell's own width, unpadded, after which
   the row flows; `Fill`, the rest), an edge (`Align::Left`/`Right`), a
   gap after it (`GAP`, 2, unless told), and how it gives way: `drops(p)`,
   `shrinks(p, floor)`, `drop_below(room)`. Two options cover what the
   old pre-passes did by hand: a `ragged` column is widened only by cells
   that something follows (a header's path spaces the counts, and a header
   with no count does not push the others'), and its cell that ends a row
   runs on into the room left, even where the column gave way for the
   counts on other rows; and a column that
   `absorbs_lead` is measured from the row's start so it ends in one place
   at every depth (the counts of nested headers stay in line).

2. **Typed cells.** A `Cell` knows its width and cuts one way: text at
   its end (`…`), in the middle (paths, identifiers), or clipped (a key
   whose first cells name it), with a tail kept whole (`(cwd)` after a
   path); styled spans and chips; a chip list (as many whole chips as fit
   and ` +N`); the framework's age cell; a snippet cut around its match.
   A row's cells are `Cells<K>`, keyed by column.

3. **One measure pass.** `Measure` goes over every row of the tree once a
   frame, rows a fold hides included (ADR 0131's rule, which the header
   alignment already followed after CSP-714), and keeps each key's widest
   cell. Schemas that share a key share the column: every kind of row in
   the left pane shares the prefix keys, so the pin marker cell is
   reserved on every row once any row is pinned and a header for no
   entity keeps a blank glyph cell once another header has one. Cells
   under keys a row's schema lacks are measured too, so a host that still
   draws part of a row reads its widths from the same pass (the Pinned and
   History rows do until CSP-752).

4. **One narrow-width policy.** A row is laid out in its room (the pane's
   width less its indent):
   1. a column below its `drop_below` room is left out;
   2. every column takes its width, a fill `MIN_FILL` (7) cells;
   3. while the row is too wide, columns give way in ascending priority,
      the later of two at one priority first and a narrowing before a
      leaving: a shrinking column narrows no more than needed and not
      below its floor (or its whole text, when shorter); a dropping one
      leaves with its gap; a column that narrowed takes back what a later
      leaving freed;
   4. the fill takes what is left, cut with `…`, and shows only with
      `MIN_FILL` cells or its whole text;
   5. a row still too wide is cut at its edge with `…`.

   The columns before the first hugging one line up: they give way by the
   widths measured over the whole tree, so every row drawn in one room
   loses the same ones and keeps the rest in place. The hugging column and
   the ones after it give way row by row. A row's last cell needs only its
   own text, not its column's pad. One fill minimum (7, the old preview
   rule: more than 8 cells left beside the 2-cell gap) replaces the three
   "room remains" cutoffs; a label, path, or project narrows to no fewer
   than 8 cells and then leaves, and a mux name narrows to 10 and stays.

5. **The tree's producer.** `tree::RowColumns<R>` is the host's half: the
   schema a row is drawn from and the row's cells, given the row's
   context. Cells do not depend on the row's width or cursor; the tree
   measures every row with a context of width 0. `TreeColumns` measures in
   its constructor and lays each row out after its indent (`INDENT`, 2
   cells a level) for `TreeView`; `line_then` draws a schema's columns and
   hands the cells left to the host, for kinds of row not yet on a schema.

6. **The left pane's schemas.** Conspectus declares the prefix
   (disclosure, kind glyph, a header's glyph, the pin marker), the group
   header (label; path or members cut in the middle with `(cwd)` kept,
   ragged and lead-absorbing; count; warning), the session row (key,
   chip, age, state; then label, `was in` note, project, hit count, and
   preview or snippet), and the mux row (name stepping 18/20/24/30 by the
   room; chips capped at two and a count; age; state; hit count; preview,
   snippet, or directory). Headers and session rows use them in every
   view; mux rows in the Mux view. A stopped mux stays a mixed-kind row
   (actions, menus, and quick select read it as before), but one that
   shows its state as a glyph is drawn from the mux schema, and the App
   gives it the tree's whole-row ghost look like every other gone row of
   a single-kind view. The PR, Pinned, History, hit, and pane rows keep
   their own cells after the prefix the framework draws until CSP-752 and
   CSP-753. Their priorities:

   | Row | Gives way first to last | Stays |
   | --- | --- | --- |
   | Header | the path narrows, then leaves | label, count, warning |
   | Session, lined up | key, chip, age | state |
   | Session, the rest | preview or snippet, `was in`, project (narrowing first), hit count, label (narrowing first) | |
   | Mux, lined up | chips narrow to one and `+N`, name narrows, age, chips | name, state |
   | Mux, the rest | preview, snippet, or directory, hit count | |

7. **The CLI keeps its own allocator.** `output::render::fit_to_width`
   shrinks, one cell at a time, the column with the most room above its
   floor (its header or 8 cells), never drops a column, prints plain text,
   and lays out the columns the operator picked with `--columns`. A schema
   drops and narrows by declared priority over styled cells measured over
   a tree. Sharing one engine would change what `conspectus table` prints
   or give the schema a second policy; the two already share the measure
   and the cut (`tuikit::text`, ADR 0155). `ColumnSpec` stays the CLI's
   registry of selectable columns.

8. **Gaps are blank.** A gutter is drawn in the default style, and a text
   cell's pad in the text's style, so a padded text cell is one run. The
   old renderers styled some gutters with the cell before or after them.

## Consequences

- At wide widths a row that fits looks as it did; two things change
  there: a stopped mux's `▣` takes the ghost look with its row, and a
  mux whose agents need two chips and a count keeps its age in line with
  the others (the chips column is as wide as the widest row's). The
  gutters between cells lose the styles some carried, which shows only
  inside the focused cursor's highlight.
- Narrow panes change by the policy: a cut label, project, path, or name
  ends in `…` instead of stopping at the pane's edge; a session row drops
  its key, then its chip, then its age; a mux row narrows its chips and
  name before its age leaves; a stopped mux's directory needs 7 cells (it
  showed from 3); a cell that cannot keep 8 cells leaves rather than show
  a fragment.
- `GroupAlign`, `MixedAlign`, `RowPrefix`, `MixedGrid`, the session,
  header, and mux span renderers, and the mux width tiers in code are
  gone; the widths are declarations in one module, the gutters a
  default. `PrAlign` stays for CSP-752.
- A consistency test reads the drawn frames at 60, 80, 110, and 160
  columns and fails when a lined-up cell moves between rows of one kind
  and depth, or when a session row lays out differently in the Sessions
  and Mux views.
- The nested-header path drift (CSP-772) is unchanged: the header label
  is still padded to one width whatever its indent. `absorbs_lead` on the
  label column is the one-line fix, left to CSP-752 with the header's
  structured cells.

## Alternatives Considered

- **Build on `palette::ColumnPlan`.** Its drops and narrowing to a floor
  are the same idea, but it measures nothing, cannot drop its fill, and
  has no flowing cells or keyed columns. Growing it into the schema would
  change the palette's API mid-story; the search rows' move (CSP-753)
  decides whether the plan becomes a schema.
- **Lay every column out row by row.** Simpler, but a long label on one
  row would drop that row's key and leave the others', so the columns
  after it would no longer line up.
- **Pad the session label to the widest in the tree.** Previews would
  line up, but most sessions have no label, and every unlabeled row would
  lose 34 cells of preview to blanks.
- **Measure the mux name** (as wide as the widest name, at most 30).
  Tighter, but it moves every Mux frame; the stepped widths keep them.
- **A fill minimum of 8.** A round number, but it hides previews the old
  rule showed; 7 is that rule.
- **Keep the stopped mux's glyph in its color** (dimming its cells, not
  the row). It would need a look for part of a row; every other gone row
  of a single-kind view is a ghost whole, and the Pinned and History rows,
  which list kinds side by side, keep their glyph's color because there
  it is the only sign of the kind.
- **Fix the nested-header drift now.** One flag, but a visible change in
  the checkout grouping that CSP-752 makes with the header's structured
  cells; this story keeps to the policy.

## Open Questions Answered

- *One schema per view or per kind of row?* Per kind of row, in every
  view that lists it: a session row is laid out alike in Sessions, Mux,
  PRs, and Search.
- *Whole-tree or visible-row measuring?* Whole tree, folded rows
  included.
- *Does the CLI share the engine?* No (decision 7).
- *What replaces the >2, >4, and >8 cutoffs?* `MIN_FILL`, 7 cells, for
  every fill; floors of 8 for what narrows and then leaves.
