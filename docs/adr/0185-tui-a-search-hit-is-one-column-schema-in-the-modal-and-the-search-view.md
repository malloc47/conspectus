# ADR 0185: A Search Hit Is One Column Schema In The Modal And The Search View

## Status

Accepted. Carries out the hit-row part of
[ADR 0183](0183-tui-rows-come-from-column-schemas-and-one-rule-decides-what-gives-way.md)
decision 6 (the Search view's hit and pane rows were the last to draw
their own cells after the prefix, as
[ADR 0184](0184-tui-pr-pinned-and-history-rows-come-from-schemas-and-headers-carry-two-cells.md)
noted) and answers the question ADR 0183 left open: the palette's column
plan becomes a column schema. Amends
[ADR 0175](0175-tui-search-is-two-palettes-and-a-hit-is-one-column-plan.md)
decision 3: `tuikit::palette::ColumnPlan` is retired, and the hit plan is
`hit_columns`, laid out by `tuikit::columns`. Keeps the search semantics of
[ADR 0147](0147-session-content-search.md) and
[ADR 0149](0149-two-level-search-current-and-wider-context.md), including
item 25's columns, widths, and drop order for the contents modal, and
[ADR 0157](0157-tui-chips-one-width-rule-and-ages-one-ladder.md)'s age cell.

## Context

One search hit was laid out by three grids: the contents modal's results
(`hit_plan`, a `palette::ColumnPlan`), quick select's entity results
(`entity_columns`), and the Search view's hit, pane, and pane-hit rows
(`render_hit_spans`, `render_pane_spans`, `render_pane_hit_spans`, with
their own widths: a name of 12 or 16 cells, `turn {:<4}`, a role of 6, a
kind of 11). The view's rows drew their cells after the prefix the
framework laid out and were cut at the pane's edge with no mark: at 110
columns a hit row stopped at `messag`, and at 130 columns no hit row
showed its snippet. Two engines did one job: the plan shared a row out
by shares and steps of its width with drop and narrow orders, the schema
measured cells over a tree with one narrow-width policy, and neither could
lay out what the other did.

Facts established while moving the rows:

- The plan's rules the schema lacked were three: a share of the row's
  width, steps by the row's width (the plan's steps read the whole row,
  the schema's the room left for the column), and a fill's own minimum
  (20 for a snippet, where the schema's fills want 7). With them, and the
  modal's cursor gutter as the row's lead, the schema lays the modal's
  results out with the same text at every width.
- The modal's rows carry a source word because they list sessions' and
  panes' hits side by side with nothing else to tell them apart. A tree
  row says what it is with its kind glyph (a pane row wears the mux's), or,
  in the Messages listing, with its chip and its tag (`turn` or `line`).
- The modal's tag is `turn N · kind`; the view's turn had a role word
  besides (`you`, `ai`, `system`). `turn 45 · you · message` is 23 cells,
  one more than the tag's widest step.
- The view's session rows are the Sessions view's rows (ADR 0147), laid
  out alike in every view (ADR 0183's per-kind answer).
- The entity rows' grid gives its label a share of what the outcome word
  leaves and drops a context below 10 cells, handing its room to the label.

## Decision

1. **The schema gains the plan's rules.** `Column::share(key, per,
   of).within(min, max)` takes a part of the whole row, `Column::row_steps`
   steps by the whole row's width whatever the columns before it take, and
   `.at_least(n)` sets the cells a fill wants before the others stop giving
   way. Both rules of the row count its lead (a cursor gutter, an indent),
   so two lists of one whole width give such a column one width.

2. **The plan is retired.** `palette::ColumnPlan`, `Column`, and
   `ColumnLayout` are removed; a palette row of several cells draws a
   column schema with the cursor gutter as its lead. One narrow-width
   policy lays out every row of several cells the framework draws.

3. **A hit's columns are one declaration.** `hit_columns(key, chip,
   shape)` in `src/tui/widgets/search/rows.rs` is ADR 0149 item 25: the
   source word (7), the chip or a pane's command (a chip's footprint), the
   name (a fifth of the row, 12 to 32 cells), the hit count or position
   (`row_steps`: 22, 20, or 14 cells), the age (4), and a snippet of at
   least 20 cells; the chip gives way first, then the source word, then
   the age, and the name narrows to 8 last. The key is the caller's: the
   modal keys it by `HitCol`, the left pane by `Col::Hit(HitCol)`. Its
   shape says which columns a list draws: `Sourced` (every one: the modal),
   `Named` (all but the source word: a pane row and a row of the Messages
   listing, after the tree's prefix), `Nested` (the tag, the age, and the
   snippet: a hit under the row that names its session or pane).

4. **A hit's cells are one builder.** `Hit::cells` gives the source word
   and the tag receding, the harness chip or the command, the name, the
   framework's age cell (`Cell::age`, tinted by the hit's recency; the
   modal's results carry it, `ContentRow::recency`), and the snippet cell
   (`snippet_cell`: inert italics, the match in the match look, Markdown
   ticks left out), which the left pane's session and mux rows use too.
   The tags are one function each (`hits_tag`, `turn_tag`, `line_tag`).

5. **The tree has no source word, and a turn no role word.** A tree row
   is known by its glyph, chip, and tag, so its schema is `Named`; the
   source word would cost 8 cells a row in a pane half the modal's width.
   A turn's tag is the modal's `turn N · kind`.

6. **Session rows keep the session schema.** The Search view's session
   rows are the Sessions view's rows and lay out alike in every view; their
   hit count and snippet are the content filter's cells, the snippet cell
   being the hit's.

7. **Quick select's entity rows keep their split.** The schema has no
   share of what the other columns leave and no fill that hands its room to
   a neighbor; the column schema's page lists both as a seam.

## Consequences

- The modal's results read as they did at every width, character for
  character. Their ages are tinted by how recent they are (they receded),
  their snippets take the panes' snippet look (inert italics, where they
  were dimmed), and the gaps between cells take no style (visible only
  inside the cursor's highlight).
- The Search view's hit, pane, and pane-hit rows take the modal's columns
  and give way in its order instead of being cut at the pane's edge: at
  130 columns a hit row shows its snippet; at 160 the tag is 14 cells
  (`turn 46 · mes…`) because the rows of a list that overflows the pane
  are 77 cells, under the 78-cell step; a pane row puts its command first
  and its hit count before
  its age, and its command is the first cell to give way. A hit reads as
  the modal's row at one width once the modal's source word has given way,
  and the chip, name, tag, and age take the same cells where it has not.
- A turn no longer says who spoke; the Preview and the viewer still do.
- `render_hit_spans`, `render_pane_spans`, `render_pane_hit_spans`,
  `append_content_hit`, `append_snippet`, `age_span`, `FLAT_LABEL_WIDTH`,
  and `PANE_LABEL_WIDTH` are gone. No row of the left pane draws its own
  cells, which is what the pointer gutter of ADR 0183 waited for (CSP-775);
  `TreeColumns::line_then` and `ColumnSchema::draw_then` stay in the
  framework for a host that does.
- Tests: the framework's rules (unit and property tests, a rustdoc
  example), the hit rows' drop order and fit at every width, a hit that
  reads as the modal's row, and the Messages listing's turn column at 60
  to 160 columns. A check that counted the turn column in bytes counts
  characters, and the showcase screenshot's pane row, a cell too narrow
  for its command at that width, is found by its glyph, name, and count.

## Alternatives Considered

- **Keep the plan and lay the view's rows out with it** (`line_then` and
  a plan after the prefix). One declaration, but two engines, a plan that
  measures nothing beside a schema that does, and rows still drawn by
  hand after the framework's prefix.
- **The source word in the tree too.** The rows would match the modal's
  cell for cell, but at 160 columns the chip would give way first and
  leave `session` where the harness was.
- **Keep the role word** (`turn N · you · message`). The widest tag step
  cuts it by one cell, and every narrower step loses the kind.
- **The modal's results keep their dim ages and snippets.** The cells
  would be two looks of one hit; the shared cells are the framework's.
- **The Search view's session rows on the hit schema.** A session would
  lay out one way in Search and another everywhere else, and lose its key,
  link glyph, project, and `was in` note.
- **The entity rows on the schema with a host-computed width.** A width
  rule that calls the host would cover them, but it is an escape hatch the
  policy cannot reason about; the two missing rules are the general form.

## Open Questions Answered

- *Does the plan become a schema?* Yes; the palette's plan is retired.
- *Do the view's hit rows show the source word?* No (decision 5).
- *Do the entity rows move?* Not yet (decision 7).
