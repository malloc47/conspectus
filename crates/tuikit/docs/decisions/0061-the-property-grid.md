# 0061: Detail rows are a property grid, and a row reports what it asks for

| | |
| --- | --- |
| Status | Accepted |
| Page | [controls/property_grid.md](../controls/property_grid.md) |

## Decision

- **Three row kinds and the grid.** A `PropertyRow` is a bold label in a
  fixed label column (14 cells; a label as wide as its column keeps a blank
  cell before the value), a glyph, the value as text or spans cut in cells
  at its end or its middle with a hint naming the host's inspect key, a
  placeholder that recedes, and an annotation in `warning`. A `LinkRow` is
  an indent, a verb in a fixed verb column (22 cells), a glyph or the
  no-value mark, the far end's label cut in the middle to the room left,
  meta, and a tone. A `DisclosureRow` is the disclosure mark, a label, a
  count, and tallies of the rows that carry a mark. `PropertyGrid` holds the
  rows and lines that are no row (a rule, a heading), draws each row through
  the cursor look, and counts its cursor over rows only.
- **A row reports an intent; the host does the effect.** `activate` (the
  host's `Enter`) and `inspect` (its inspect key) return `Copy` with the
  property's whole value, `Inspect` when it is drawn cut, `Toggle` for a
  section header, `Follow` for a link, or nothing; a placeholder copies
  nothing and an inert link follows nowhere.
- **The row cuts; the data keeps the whole value**, so what is shown, copied,
  and inspected cannot disagree.
- **Generic tones.** `Primary` (`link`, bold), `Quiet` (`inert`), `Alert`
  (`warning`, bold, led by the warning mark, which survives no color), and
  `Inert` (the whole row muted). `Tones` maps them to the theme, and a host
  may replace a tone's look with a role of its own; which of its things
  takes which tone is the host's.
- **Meta is kept.** A link's label leaves room for its meta while that keeps
  eight cells of label; with less, the meta runs past the width for the
  host's wrap to carry.
- **No column schema.** The grid has two fixed columns with nothing to
  measure, and its property rows wrap a long value where a schema row would
  give way on one line.

## Rationale

The host cut values in its row builders, so the hint, the copy, and the
value could disagree, and the detail rows drew their own cursor, labels,
and folds. Rows that report what they ask for keep every effect with the
host and make the grid testable alone.

## Consequences

- The explorer feeds on these intents (record [0062](0062-the-explorer.md)).
- A measured label column is a seam on the page.

## Alternatives considered

- **Keeping the cut in the builders.** The copies of "cut, compare, keep the
  whole" stay, and a row could not tell whether its hint is true.
- **Dropping the meta when the label would fall under eight cells.** A narrow
  pane would show no meta when the user asked for it.
- **A column schema for the grid.** Every value would be one line.
- **A mark the host chooses for alerts.** The no-color guarantee would
  depend on every host remembering it.

## Origin

- host ADR 0187, "Detail rows are a property grid, and a row reports what it
  asks for": decisions 1 to 6 and 9.
