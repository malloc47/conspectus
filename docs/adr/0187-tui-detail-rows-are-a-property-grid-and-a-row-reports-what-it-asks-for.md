# ADR 0187: Detail Rows Are A Property Grid, And A Row Reports What It Asks For

## Status

Accepted. Carries out the `PropertyGrid` part of the data-display row in
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md)'s control
set, applying house rules 1, 2, 10, 13, and 14, and extends
[ADR 0150](0150-tui-framework-workspace-crate.md) with the framework's
`property_grid` module. Amends
[ADR 0075](0075-edge-state-visual-language.md) decision 1 (an unresolved
row is one line, and its provenance and confidence are edge meta) and
[ADR 0074](0074-detail-pane-related-entities-flatten.md) decision 3 (the
Other header's arrow takes the `disclosure` color).
Amends [ADR 0154](0154-tui-glyph-marks-vetted-table-and-scan.md):
`marks::disclosure` picks the folding arrow, and the host's mark guard
also refuses a string a bare mark leads. Keeps
[ADR 0159](0159-tui-lists-show-their-cursor-one-way.md)'s cursor look,
which the right pane already had, and does not use
[ADR 0183](0183-tui-rows-come-from-column-schemas-and-one-rule-decides-what-gives-way.md)'s
column schemas (decision 6).

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/07-chrome-and-explorer.md`,
sections 3.10 to 3.12, 3.17, 8, and 9) found the right pane's rows drawn
by six functions in `ui/right_panel.rs`:

- **Field rows** padded a 14-cell label with `format!`, so a label of 14
  cells or more ran into its value (`current_commitabc1234`,
  `last_message_previewI expanded…`). Seven builders cut long text at 48
  characters and kept the whole value beside it by hand, and the hint
  `(truncated · o)` typed the key.
- **Related rows** came in four variants. Three shared a row renderer;
  the unresolved stub was one `format!` string that typed `—` and wrapped
  its evidence over two or three rows.
- **The Other header** typed its text and drew its arrow in the header's
  gray, where the left pane draws folding arrows in `disclosure`.
- **Enter and `o`** looked the cursor's field up again to copy or open
  it (the defect CSP-713 fixed by carrying the value on the row).
- **The Preview** re-typed the 14-cell label column five times.

After CSP-727 every row already went through `selection::row`, so the
highlight extent and the unfocused cursor needed no change here. Outside
the right pane, nine drawing sites still typed `…` as a mark.

## Decision

1. **`tuikit::property_grid`.** Three row kinds and the grid:
   - `PropertyRow`: a label in a fixed label column (`LABEL_WIDTH`, 14), a
     glyph, the value as text or spans, a cut (`columns::Cut`) to a number
     of cells, a placeholder flag, a hint, and an annotation.
   - `LinkRow`: an indent, a verb in a fixed verb column (`VERB_WIDTH`,
     22), a glyph (the no-value mark when there is none), a label cut in
     the middle to the room left, meta, and a `Tone`.
   - `DisclosureRow`: the disclosure mark, a label, a count, and tallies
     of rows that carry a mark.
   - `PropertyGrid` holds the rows and lines that are no row (a rule, a
     heading), draws each row through `selection::row`, and counts its
     cursor over rows only.

2. **A row reports an intent; the host does the effect.** `activate`
   (the host's Enter) and `inspect` (the host's inspect key) return an
   `Intent`: `Copy { label, text }` with a property's whole value,
   `Inspect { label, text }` when the property is drawn cut, `Toggle` for
   a section header, `Follow` for a link, or nothing. A placeholder copies
   nothing, and an inert link follows nowhere. The host writes the copy
   to its clipboard sink and opens an inspected value in its value modal
   (a `Pager`).

3. **The row cuts, the data keeps the whole value.** `CoreField` holds
   the whole value and `cut_at`, the cells the pane shows
   (`LONG_VALUE_CELLS`, 48, for titles, messages, commands, and URLs).
   The row cuts it, draws the hint, and hands the whole value to a copy
   or an inspect, so the three cannot disagree. The hint names the key
   `keymap::INSPECT_KEY` binds, so the hint and the binding cannot drift
   apart. `ExplorerRow::NodeField` carries its field, so the Preview
   reads the field's typed value from the row too.

4. **A label as wide as its column keeps a blank cell.** A label of 14
   cells or more is followed by one space, then the value.

5. **Tones are generic; the host maps its roles.** `Tone::Primary`
   (`link`, bold), `Quiet` (`inert`), `Alert` (`warning`, bold, led by
   `marks::WARNING` in its indent, the signal that survives no color),
   and `Inert` (the whole row in `muted`). `Tones` maps the four onto
   styles from the theme's tokens, and the host replaces the quiet and
   alert looks with its `edge_alt_of` and `edge_conflict` roles, so those
   keys keep working. Which edge state takes which tone is the host's: a
   resolver winner is primary and flush under the rule, an alternate
   quiet and a conflict alert under the Other header (four cells in), an
   unresolved stub inert.

6. **Meta is kept.** A link's label leaves its meta room while that keeps
   `MIN_LINK_LABEL` (8) cells of the label; with less, the label takes the
   room and the meta runs past the width, where the explorer's wrap
   carries it to the next row, as it did. With edge meta on (`E`) the
   operator asked to see it, so it is never dropped.

7. **An unresolved stub is an inert link row.** Its label is what the
   evidence names (native id, harness, path, or the node type) and it is
   cut in the middle like any link label, so the row is one line. Its
   provenance, confidence, and state are edge meta, shown with `E` as a
   neighbor row shows its link's. The Preview shows the evidence whole
   when the cursor is on it.

8. **The section header shares the tree's mark, not its row.**
   `marks::disclosure(open)` picks the arrow for `tree::disclosure` and
   `DisclosureRow` alike, both in `disclosure`. The left pane's group
   header stays a column-schema row: it has a path column, counts lined
   up over the whole tree, and a lead the tree measures, none of which a
   section header in a list of properties has.

9. **No column schema.** The grid's columns are two fixed widths with
   nothing to measure, and its property rows wrap a long value while a
   schema row is one line that gives way. A measured label column is a
   seam on the control page.

10. **The Preview's lines are grid rows.** The neighbor's fields, its
    `edge`, and its `evidence` lines are property rows in a grid with no
    cursor. The `neighbor` and `unresolved` headings stay headings, their
    names two cells left of the value column, as they were.

11. **Every typed mark is the table's.** The nine `…` marks name
    `marks::ELLIPSIS`, and the host guard refuses a literal that a bare
    mark leads (`… 3 more`) or that is one with spaces around it (` …`).

## Consequences

- Visible, in the right pane only: labels of 14 cells or more keep a
  space before their value, so a value that no longer fits beside its
  label (a long path) starts on the next row; an unresolved stub is one
  row with its evidence cut in the middle, and shows its provenance with
  `E`; the Other header's arrow is cyan, not gray, and not bold; Enter on
  a placeholder (`—`, `— (unknown)`) no longer copies it or says
  "copied". Every other frame of the right pane is unchanged, and the
  Preview's edge and evidence rows differ only in the style of their
  blank lead cells.
- The explorer's state machine (cursor, zones, drill stack) is unchanged
  and still the host's; the generic `Explorer` builds on the grid's rows
  and intents.
- Tests: the framework's grid tests cover each row's layout, cuts, hint,
  tones, the shared mark, the cursor look in both focus states, lines
  that are no row, and every intent. The host's adapter tests cover the
  field cut and the inspect key, the label gap, placeholders, the edge
  roles, the Other header's counts, unresolved stubs, and a vanished
  link; the app tests copy and inspect every shown field and copy
  nothing from a placeholder.

## Alternatives Considered

- **Keep the cut in the builders**, with the row taking a shown value and
  a whole one. Fewer changes, but the seven copies of "cut, compare,
  keep the whole" stay, and the row could not say on its own whether its
  hint is true.
- **Drop the meta when the label would fall under 8 cells**, as the
  inventory described. Rows would stay one line, but at a 60-cell pane
  `E` would show no meta at all; the operator asked to see it.
- **Keep the unresolved stub's wrapping and its provenance.** Byte for
  byte as before, but the one link row that wraps, with the one shape no
  other link row has.
- **Convert the Preview headings to property rows.** Their names would
  line up with the fields under them, but a 34-cell name that fit beside
  its heading at a 48-cell pane would move to its own row.
- **A whole `DisclosureRow` in the left pane.** The left pane's header is
  a schema row measured over the tree; making it a grid row would lose
  that alignment.
- **A column schema for the grid.** Reuse of the measure and the
  give-way policy, but the wrap of long values and the fixed columns are
  what the pane wants, and a schema would make every value one line.
- **A host-chosen alert mark.** A `prefix` span on the row would let a
  host pick the mark, but then the no-color guarantee would depend on
  every host remembering it.

## Open Questions Answered

- *Do copy and inspect live in the framework?* The intents do; the
  effects (the clipboard sink, the value modal) stay with the host.
- *Is the left pane's group disclosure the same control?* It shares the
  mark and its look, not the row (decision 8).
- *Does the right pane use column schemas?* No (decision 9).
- *What happens to the unresolved row's provenance?* It is edge meta
  (decision 7).
