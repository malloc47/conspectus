# ADR 0159: Lists Show Their Cursor One Way And Step By Their Kind

## Status

Accepted. Applies house rules 1, 2, and 10 of ADR 0151 and extends ADR
0150 with the framework's `selection` module. Carries out the step ADR
0156 decision 2 left for this module: the menus' hand-rolled clamps are
gone and the menus wrap. Widens ADR 0032's `selection_active` and
`selection_inactive` keys from the panes' cursor rows to every list's.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/`) found the TUI's lists showing and
moving their cursor in ways that shared nothing:

- **Three cursor glyphs.** The row, mux, and worktree menus drew `►`
  (typed three times beside the unused `marks::POINTER`); Controls, the
  picker, the search lists, and the forms drew an ASCII `> ` (ten
  literals in five files); the checklists under Controls and Clean up
  drew the third-party widget's own accent `>`; Messages and both panes
  drew none.
- **Four highlights.** The menus made the cursor row bold; Controls, the
  picker, and the search lists reversed it with a hard-coded modifier;
  the checklists colored only the glyph; Messages and the left pane read
  the theme's `selection_active`. Only those two read the theme, so an
  operator's `[tui.theme] selection_active` reached two lists of about a
  dozen.
- **Partial rows.** The explorer reversed only a field's value, or a
  related row's verb, glyph, and label but not the space between them.
- **An invisible unfocused cursor.** The left pane kept a bold cursor row
  when it lost focus; the explorer drew nothing, so a reader could not
  see where `Tab` would land.
- **Menus that stopped.** The menus and Messages clamped at their ends
  while Controls, the picker, the checklists, and the search lists
  wrapped.

## Decision

1. **`tuikit::selection` owns the cursor's look and the ends rule.** Its
   items: `Cursor` (off, active, inactive), `GUTTER_WIDTH`, `gutter`,
   `highlight`, `row`, `ghost_style`, `ghost`, `pending`, and `Ends`
   (`Wrap`, `Clamp`). A list draws each row with `selection::row`.

2. **The marker.** Every row starts with a 2-cell gutter:
   `marks::POINTER` and a space on the cursor row, focused or not, and two
   spaces on every other row, so no text moves when the cursor does and
   the cursor row is found with no color and no reverse video. The marker
   takes the row's look, not the accent: an accent cell inside a reversed
   row would read as a second cue, the pane title's focus marker.

3. **The highlight.** The cursor row takes the theme's selection pair on
   every span, gutter included: `selection_active` while the list takes
   keys, `selection_inactive` while it does not. A filled span (anything
   with a background or drawn reversed: a chip, a form field's fill, its
   text cursor cell) keeps the look it has on every other row, so it
   still reads as a chip or a field. The row's own colors and its muted
   text stay under the highlight, as they did in the left pane; the
   search lists, which used to drop the dim on their cursor row, now keep
   it. The highlight covers the spans a row is made of and is not padded
   to the list's width, which is the left pane's look; rows that already
   run to the edge (the search lists) still do.

4. **Focus.** A list is focused while it takes the cursor keys: the
   focused pane, an open menu, picker, checklist, palette, or form. The
   unfocused pane and the Controls list behind its open checklist or
   editor keep their cursor row marked in the inactive look (house rule
   10).

5. **The ends.** `Ends::Wrap` for short choice lists: the row, mux, and
   worktree menus, Controls, the picker, the checklists, quick select,
   content search and its options, and a form's ring of fields.
   `Ends::Clamp` for scrollable content: the tree, the explorer, and the
   Messages log. A value that cycles in place (a toggle's states, the
   harness choices inside one field, the tab strip) has no cursor row and
   keeps stepping with `scroll::wrap_step`.

6. **Where the gutter goes.** The explorer's rows already led with two
   blank cells (the field labels, the related rows' indent), so the
   gutter takes them and no column moves, except that a conflict row's
   `▲` now comes after the gutter instead of in it. The menus, Controls,
   the picker, the search lists, the checklists, and the forms already
   drew a 2-cell cursor lead. Messages gains the gutter and its columns
   move two cells right. The left pane's tree draws the highlight without
   a gutter: its rows begin with disclosure and kind columns that
   `docs/tui-columns.md` documents, and the gutter belongs with the tree's
   column schema when that lands.

7. **Guards.** The framework's raw-style guard fails on
   `Modifier::REVERSED` in framework source outside four reviewed files
   (the theme's defaults and grammar, the scrollbar that removes it, and
   the highlight that looks for it). The host's consistency suite fails on
   a string that starts with `> ` or a `Modifier::REVERSED` in its drawing
   code outside four reviewed uses that draw no list cursor (the snapshot
   writer, two text cursor cells, and the reader's current find match),
   checks that every list modal marks one row in its gutter and
   highlights it, that the marker alone finds the row with the selection
   pair emptied, that the short choice lists wrap, and that the unfocused
   explorer keeps the inactive pair.

8. **The checklist draws its own rows.** The upstream multi-select keeps
   only its cursor and checked set until the framework's `Checklist`
   replaces it; its rows are drawn through `selection::row`, and a label
   too long for the row is cut with `…`.

## Consequences

- Visible: every list's cursor is `►` in its gutter and the row is
  reversed and bold (the default pair); the menus are no longer bold-only,
  and Controls, the picker, the search lists, and the forms no longer show
  `>`. The menus wrap at their ends. The right pane marks its cursor row
  in bold while the tree has focus, and reverses the whole row (not only
  a value) while it has focus. Messages marks its selected entry and its
  columns move two cells right. The search lists' cursor row keeps its
  dim context and adds bold. A form's focused row reverses its label and
  its marks around the field, whose fill is unchanged. The conflict mark
  in the explorer's Other zone moves two cells right. Clean up and the
  checklists cut long labels with `…`.
- An operator who sets `selection_active` or `selection_inactive` sees it
  in every list.
- Later controls (`ActionMenu`, `Checklist`, `Palette`, `SettingsPanel`,
  `Form`, `TreeView`, `PropertyGrid`, `Explorer`) build their rows on
  `selection::row` and step with `Ends`, so none of them changes the look
  again.

## Alternatives Considered

- **An accent-colored marker.** The pane title's focus marker is cyan; a
  cyan cell inside a reversed row reads as a block of color and as that
  other cue. The marker takes the row's look.
- **A highlight padded to the list's width (a bar).** Conventional, but it
  changes the left pane's look and every frame in `docs/tui-columns.md`.
  Left as a seam on the control page.
- **Dropping the dim on the cursor row**, as the search lists did. The
  left pane keeps it, and it keeps the row's hierarchy legible; one rule
  is the left pane's.
- **The gutter in the left pane now.** It moves every column by two cells
  and every documented frame; the tree's column schema is where the
  tree's columns are decided.
- **Messages wrapping.** It is a log, and house rule 2 says logs clamp.
- **A form highlighting only its label.** It would keep the field's fill
  as the only cue, but it is a fifth look; the fill keeps its look inside
  the highlight anyway.
- **Keeping the third-party checklist's rendering with a buffer pass on
  top.** Painting the highlight after the widget drew would keep its
  accent `>` and hand-place the marker; drawing the rows is simpler and
  is what `Checklist` will do.

## Open Questions Answered

- *Does the marker show on an unfocused list?* Yes, with the inactive
  pair.
- *Does a form's highlight cover its value?* The row is highlighted; the
  value's fill and its text cursor keep their look.
- *Do in-place value cyclers go through `selection`?* No: they are not
  lists and keep `scroll::wrap_step`.
