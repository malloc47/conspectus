# ADR 0174: Palettes Are One Control, A Query Over Results The Host Ranks

## Status

Accepted. Carries out the `Palette<R>` control of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md), applying its
house rules 1, 2, 4, 5, 8, 10, 12, 13, 14, and 15 to every palette, and
extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the
framework's `palette` module. Amends
[ADR 0158](0158-tui-modals-one-frame-and-named-sizes.md): the picker takes
the `PALETTE` size, not `WIDE`. Amends
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md):
`PALETTE_KEYS` (`PaletteKey`) and `PALETTE_OPTION_KEYS`
(`PaletteOptionKey`) join the standard tables, and the host's all-alias
`PICKER_KEYS` is gone.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/04-menus-pickers-dialogs.md`, section
3.4, and `05-search-help-viewers.md`, sections 2.7, 2.8, and 6) found the
picker (retargeting a pin, the search history), quick select, and content
search to be one control drawn three ways: a query line, a ranked list
with a cursor, a status row, a generated footer, an options pane, a
debounce for a slow provider, and an outcome. Between them:

- **Three cursor rules after an edit.** The picker clamped its cursor to
  the end of the narrowed list, so the row under it changed as the list
  narrowed; quick select kept the cursor's position unless the list
  became shorter than it, then went to the top; content search went to
  the top on every edit.
- **A picker with no keys shown.** Its table was all aliases, so it drew
  no footer, and it had no paging.
- **Two sizes for one kind of list.** The picker filled the screen like
  the help (`WIDE`, so its frame kept its size while it was filtered); the
  search overlay took the palette size.
- **A domain enum in the widget.** `PickerPurpose` named what a pick
  meant and built the host's messages inside the widget file.
- **Rows cut at the frame.** The picker's labels and details were clipped
  by the paragraph instead of ending in `…`.
- **A dead height cap** (inventory D20), already removed when the picker
  took a named size; this decision confirms none came back.

## Decision

1. **`tuikit::palette` owns the palette.** Its items: `Palette<R, M, A>`,
   `PaletteRow`, `RowContext`, `Item<T>`, `every_word`, `Options`,
   `Debounce`, `Count`, `PaletteAction`, and `PALETTE_SIZE`
   (`ModalSize::PALETTE`).

2. **Rows are the host's.** A row implements `PaletteRow`: `cells` lays
   it out for the width it is told (after the cursor gutter, and after the
   scrollbar's column when the results overflow), knowing whether the
   cursor is on it, the query, and the theme; `haystack` is what the
   built-in filter matches. `RowContext` is built only by the palette, so
   it can carry more (a plan of columns) without breaking a row. The
   palette draws each row as `selection::row` over the cells cut to the
   row with `…` in cells, so every palette's rows look and move alike.
   `Item<T>` is the built-in row: a label, an optional detail after two
   spaces and the separator mark, receding, and the host's value.

3. **Ranking is the host's.** A palette over known candidates takes a
   ranker (`ranked_by`: the query and every row in, the indices to list
   out, best first) and reranks on every edit and whenever its rows
   change; `every_word` is the built-in ranker (every word of the query in
   the haystack, ignoring case, in the host's order). A palette fed by a
   provider takes no ranker and lists the rows it is handed with
   `set_rows`.

4. **One cursor rule.** An edit (typing, deleting, `set_query`), a scope
   switch, and an option change put the cursor on the first result, the
   best one. New rows keep the cursor where it was while that is still a
   result and put it on the first otherwise, so streamed results do not
   move it. The arrows and `Ctrl-P`/`Ctrl-N` wrap (house rule 2);
   `PgUp`/`PgDn` move by the rows shown and stop (house rule 12).

5. **One table, the host's verbs.** `PALETTE_KEYS` binds `Ctrl-C`
   (dismiss), the arrows and their chords, `PgUp`/`PgDn` (aliases),
   `Enter` (pick), and `Esc` (close); every other key edits the query, so
   `q`, `j`, and `k` type. `PaletteKey::Scope` and `PaletteKey::Options`
   are in the enum but in no standard row: a palette with a scope or
   options includes the standard table and binds them under its own verbs
   (`Tab`, `Ctrl-O`). A host with actions the palette does not know
   writes its table over its own enum and implements `PaletteAction`
   (which of its actions are the palette's); it resolves its own first
   (`Palette::host_action`) and hands the rest to the palette, which
   swallows a host action rather than typing it. The footer is generated
   from whichever table the palette resolves with.

6. **One look, in the palette frame.** Inside `PALETTE_SIZE`, which fills
   its extent so the frame keeps its size as the results narrow: the
   query after its prompt (`/`, in `accent`) with its caret while the
   results have the keys; the status row the host sets, receding; the
   results, or a note for an empty list (`empty_note` while nothing is
   typed, `no_match_note` otherwise), receding; the footer, receding,
   keeping the bottom rows (at most four, leaving the query, the status
   row, and one result above it). The title is the name, each scope
   strip (`SegmentedTitle`), and the count (`(listed/total)`,
   `(listed)`, or none), two cells apart. The results scroll in a
   `ScrollWindow` the palette keeps between draws, by the follow rule of
   ADR 0156, with the shared scrollbar when they overflow.

7. **Scopes and options belong to the palette.** The first scope strip is
   the palette's: its key switches it to the next choice, wrapping;
   further strips are the host's to set. The options pane is a list of
   named on/off settings drawn in the results' place under the query and
   the status row, with its own table (`PALETTE_OPTION_KEYS`: the arrows
   and `k`/`j` move, `Space` and `Enter` toggle, `Esc` and `Ctrl-O` go
   back, `q` and `Ctrl-C` dismiss); the query loses its caret while it is
   open. The host reads and restores the settings.

8. **The debounce is a timer the host polls.** A debounced palette marks
   its `Debounce` on every edit, scope switch, and option change; the
   host polls `take_due(now, force)` from its loop, which answers `true`
   once per rest (or at once with `force`), starts its search with what
   the palette shows, and hands the results back with `set_rows`. What
   runs the search, and on which thread, is the host's; the framework
   starts nothing.

9. **The picker moves onto it.** The retarget picker and the search
   history picker are `Palette<Item<String>, Msg>`s built by
   `picker::retarget` and `picker::search_history`; their pick closures
   map an item's value to `Msg::CommitRetarget` or
   `Msg::ShowSearchHistory`. `PickerPurpose`, `PickerState`,
   `PickerWidget`, their overlay impl, and `PICKER_KEYS` are deleted;
   `retarget_candidates` and the history items stay in `conspectus`, and
   the picker takes the palette size, so it and the search overlay are
   one size.

## Consequences

- **Visible changes** (the CSP-742 summary lists them): the picker shows
  the palette footer (`↑/↓ move · Enter pick · Esc close`) and takes
  `PgUp`/`PgDn`; it takes the palette size, so at 100x30 it is 85 by 24
  with the panes showing around it instead of 96 by 28, and at 60x20 it
  is the full 60 columns wide; a list taller than the frame shows the
  shared scrollbar and scrolls only as far as the cursor needs; long
  candidates end in `…`; each edit puts the cursor on the first match;
  the title's count sits two cells after the title; `Ctrl-C` answers
  `Dismiss`, which closes it as `Close` did since it is never a step. At
  40x10 the footer takes a row, so six candidates show instead of seven.
  The query's caret was already drawn (ADR 0165).
- **What the search overlay inherits** when it moves onto the palette:
  its query, title, strips, status row, options pane, debounce, and
  footer map onto the palette's without a visible change, and its tables
  stay over `SearchKey` through `PaletteAction`. Three things change
  unless that story keeps its own: quick select's cursor goes to the first
  result on every edit (it kept its position), the results keep their
  window instead of re-following from the top on each frame, and an
  overflowing list shows the shared scrollbar.
- **A command palette** is an instance over rows built from a key table's
  shown rows, filtered with `every_word`; a row preview and a plan of
  columns have seams on the control's page.
- **Footers have two looks.** Menus, dialogs, and checklists draw theirs
  plain under a blank row; forms and palettes draw theirs receding with
  no blank row. This decision keeps the search overlay's look for the
  palette.

## Alternatives Considered

- **An options hook that pushes a step** (a checklist opened over the
  palette). Smaller, but the search overlay's options pane draws in the
  results' place under its query and status row and applies each change
  as it is made; a step would be a second frame with a confirm.
- **The bottom-anchored window**, re-followed from the top on every
  frame as the picker and the search lists did. It matches today's frames
  until the cursor moves back up a scrolled list, where it drags the
  list along; every other framework list keeps its window (ADR 0156,
  ADR 0171, ADR 0173).
- **The picker's clamp, or quick select's keep-unless-past-the-end.**
  Either leaves the cursor on a row that has nothing to do with what was
  typed; a ranked list's first result is the one the query asked for.
- **The picker keeps `WIDE`.** No size change for it, but two sizes for
  one control; the palette size also keeps its frame as the list
  narrows.
- **`Tab` and `Ctrl-O` in the standard table with generic verbs.** Every
  palette would show them, and the verbs (`search current/all`) are the
  host's words.
- **Host keys in a second table** the host's overlay resolves first,
  with the palette knowing only `PaletteKey`. No trait, but a host's
  footer would have to be stitched from two tables, which reorders the
  search footer.
- **A ranker that answers match ranges** with its indices. It saves a
  second search for a row that marks what matched, but the picker marks
  nothing; rows get the query, and the seam is on the page.

## Open Questions Answered

- *Which cursor rule after an edit?* The first result.
- *Does the palette rank?* No: a ranker the host supplies, the built-in
  word filter, or results the host sends.
- *Where does the options pane draw?* In the results' place, inside the
  palette.
- *What size is a palette?* The palette size, the search overlay's,
  unless the host says otherwise.
