# ADR 0184: PR, Pinned, And History Rows Come From Schemas, And Group Headers Carry Two Cells

## Status

Accepted. Extends
[ADR 0183](0183-tui-rows-come-from-column-schemas-and-one-rule-decides-what-gives-way.md)
to the PR, Pinned, and History rows (its decision 6 left them drawing
their own bodies after the prefix), refines its `Cap` and `absorbs_lead`
rules (decisions 1 and 4), and resolves its nested-header consequence.
Amends [ADR 0131](0131-pr-titles-and-the-pr-row-layout.md): the `#N`
label of decision 2 is decided while PRs are filed, not by rewriting the
built rows, and decision 4's content gives way by the one policy. Keeps
[ADR 0144](0144-pinned-and-history-views.md)'s and
[ADR 0145](0145-state-glyphs-and-words-in-the-left-panel.md)'s columns,
words, and dimming. Replaces the joined header string of
[ADR 0062](0062-workspaces-view-polish.md) and
[ADR 0064](0064-hybrid-sessions-graph-grouping.md) with two cells.

## Context

After ADR 0183 the left pane's group headers, session rows, and mux rows
were laid out from column schemas, and three kinds of row were not:

- A PR row padded its label to a tree-wide width its own pre-pass found
  (`PrAlign`) and typed its gutters inline. Its label became `#N` under a
  group that names the forge repo by a rewrite of the built rows, and the
  header gained the repo by prefixing its string. In a narrow pane the
  label stayed whole and the state glyph was cut off at the edge.
- A Pinned or History row read three widths back from the measure pass
  (`MixedWidths`) and drew its body itself: its chip left first, its
  content showed from one cell, and its state word and age were cut off
  at the edge of a narrow pane.
- A group header was one string (`GroupRow.display_path`): `slug  path`,
  `name  members  (provider)`, `path  no checkout`, `path (linked)`.
  `compact_path_label` and `compact_path_secondary` split it again on
  the first two spaces, or on a checkout's `(marker)`, to find the bold
  name and the dim path.
- A nested header drew its path two cells right of its parent's per level
  (CSP-772): its label was padded to one width whatever its indent.

## Decision

1. **The PR row is a schema.** The label (as wide as the widest in the
   tree, one cell from what follows), the tracking mark, the age, and the
   state glyph line up; the local branch (at most 24 cells) and the title
   follow. The label is structured: a row carries the forge repo it
   names, or none under a group that names the repo, and its number,
   which is the label cell's tail, so a narrowing label cuts
   `owner/repo` and keeps `#N` whole. The builder files every PR under its
   bucket first, notes each bucket's forge repos, and then builds the rows
   and headers knowing which groups name their repo.

2. **The Pinned and History row is a schema**, every cell lined up: the
   label (cut at 28, ending in one column at every depth), the state word
   (at most 9 cells), the age, the chip (one width across the tree), and
   the content. A row for something stopped, removed, missing, planned,
   or unknown wears the ghost look from its label on, gaps included, so
   its disclosure, kind glyph, and pin marker keep their looks (in a list
   of mixed kinds the glyph is the only sign of a row's kind). A row
   awaiting the hand-off shows the spinner in place of its word, padded
   to the word, so the column holds still.

3. **Their priorities**, in ADR 0183's one policy:

   | Row | Gives way first to last | Stays |
   | --- | --- | --- |
   | PR, lined up | label narrows (its number stays whole), age, tracking mark | label, state |
   | PR, the rest | title, branch (narrowing first) | |
   | Pinned or History | chip, content, label narrows to 8, age | label, state word |

   The chip leaves before the content because the content is the only
   description most kinds of pin have (a worktree, a repo, a PR has no
   chip); the content, like every fill, shows only with 7 cells or its
   whole text.

4. **Three framework rules.** A capped column counts each cell at most
   its cap in the measure and cuts it to the cap, so a capped column that
   absorbs the lead ends where its capped cells do. A row whose lined-up
   columns include one that absorbs the lead gives way as a row with no
   lead would in the whole width, and only that column is narrower by the
   lead: rows at every depth lose the same columns at the same widths, and
   a deeper row's cell may narrow below its floor. `draw_split` and
   `TreeColumns::line_with_look` part a row where a column starts, for a
   look on part of a row.

5. **A group header carries two cells.** `GroupRow` has a `label` (the
   bold name) and a `detail` (the dim text after it), which every builder
   fills: `GroupRow::at_path` names a header for a path by its last part
   and puts the path after it, `GroupRow::named` is a bucket with nothing
   more, `workspace_header` gives a workspace's name and then its members
   and provider, a checkout's marker ends its name, and a PR group that
   names its forge repo leads with it and keeps its path (and
   `no checkout`) after. Nothing splits a header's text again. Quick
   select lists a header by its text as the row reads it (`GroupRow::text`).

6. **Nested headers line up.** The header's label absorbs the indent and
   its path is measured plainly, so paths start in one column and counts
   end in one at every depth (CSP-772).

## Consequences

- At wide widths every row reads as it did, but for nested headers,
  whose paths now start in their parents' column (the count column did
  not move in the showcase world), and blank cells: the gaps of PR and
  Pinned rows lose the styles they carried, which shows only inside the
  focused cursor's highlight.
- Narrow panes change by the policy: a PR row keeps its state glyph and
  its number, a Pinned or History row keeps its name and state word, and
  content, titles, and directories show only with 7 cells or their whole
  text. In a narrow pane a nested header's count stays in its parent's
  column when the path gives way (it moved two cells per level).
- `PrAlign`, `render_pr_spans`, `MixedWidths`, `render_mixed_spans`,
  `compact_path_label`, `compact_path_secondary`,
  `format_workspace_display`, and `workspace_header_label` are gone.
  Only the Search view's hit and pane rows still draw their own cells
  after the prefix (CSP-753).
- An unchecked-out repo group whose PRs come from several forge repos
  keeps its path as its name (`~/repos/project  no checkout`), as before.
- A consistency test checks the PR, Pinned, and History views' lined-up
  cells and every header's path at 60, 80, 110, and 160 columns.

## Alternatives Considered

- **The forge repo and the number in two columns.** Numbers would line
  up in their own column, but a row under a group that names its repo
  would be padded by the repo column's width wherever another group keeps
  its repos, which moves wide frames.
- **A label cut at its end.** `conspectus/pr…` loses the number, the
  PR's identity.
- **Drop the forge repo whole when the label narrows.** Needs a cell
  that shows its text whole or not at all; a narrowing label reads as the
  same label, shorter, as every other cut does.
- **The content before the chip in Pinned.** As a session's preview goes
  first; but worktree, repo, and PR pins would keep only their name.
- **The tree's whole-row ghost look for dimmed Pinned and History rows.**
  Simpler, but the kind glyph would lose its color (ADR 0183's
  alternatives).
- **Each row in its own room when it absorbs the lead.** A deeper row
  reaches its floor first, drops a column the top row keeps, and takes
  cells back, so the columns after it move.
- **Quick select keeps the bare path.** It needs a third field or a
  heuristic join of the two; the header's text is what its row shows.

## Open Questions Answered

- *May the row model change to structured cells?* (Inventory slice 02,
  question 8.) Yes: headers carry a label and a detail.
- *Line nested headers up, or record the offset as intended?* Line them
  up (CSP-772).
