# ADR 0182: The Tree Keeps Its State By Identity, And Draws Lines Its Host Lays Out

## Status

Accepted. Carries out the first step of the `TreeView<R>` row of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md)'s control
set (the tree lands with app-rendered lines, then each view moves to a
column schema), applying its house rules 2, 10, 12, 13, and 14, and
extends [ADR 0150](0150-tui-framework-workspace-crate.md) with the
framework's `tree` module. Amends
[ADR 0031](0031-tui-filter-view-switching-and-per-view-state.md): the
per-view expanded set, selection, and left-panel scroll are one
`TreeState` that the view's slot saves and restores whole, with the
position its selection showed at. Amends
[ADR 0156](0156-tui-scroll-window-follow-rule-and-viewports-by-value.md)
decision 7: the tree's window is kept in its `TreeState`, the tree's draw
returns it, and page keys move by it without carrying a height. Amended by
[ADR 0183](0183-tui-rows-come-from-column-schemas-and-one-rule-decides-what-gives-way.md):
the producer can be `TreeColumns`, which lays each row out from a column
schema the host names for it; group headers, session rows, and mux rows
are laid out that way.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/02-left-pane-tree.md`, sections 0 to
3, 6, 7, and 11) found the left pane's tree to be a function, not a
control:

- **Loose state.** Selection, the expanded set, the scroll window, and
  the tiebreaker that picks between two copies of a row listed twice were
  four fields on `App`, mutated by `app/tree.rs`, the reducer, the
  explorer's mirror, and jump-to, and saved per view as three separate
  slot fields (the tiebreaker was not saved, so it leaked from one view
  into the next).
- **Duplicated rules.** `set_data` and `set_tree` each carried the
  same selection-retention block; expanding, collapsing, and toggling each
  re-scanned the rows for "is the selected row expandable"; jump-to and
  the explorer's mirror each walked the ancestors of a row to open them.
- **A retention that forgot the copy.** The retention block found where
  the selection had been with the first copy of its id in the tree, so a
  refresh moved a cursor standing on the second copy of a duplicated row
  up to the first, although its comment said it should not; and the
  window followed the first copy wherever the cursor was.
- **Paging carried a height.** `Msg::PageDown(u16)` carried the height the
  runtime read back from the App, which already kept the window the tree
  drew.
- **Lookups in the renderer.** Laying a row out read the snapshot's pin
  markers, its node presence (the ghost look), and whether the row awaited
  a hand-off refresh, once per visible row per frame.
- **No virtualization.** Every visible row was laid out every frame and
  the paragraph widget clipped all but the window.

`RowKind` and `RowId` are the host's central entity index (header counts,
the preview, quick select, the keymap, and actions read them), so a
framework control cannot own them.

## Decision

1. **Rows through a trait.** `tuikit::tree::TreeRow` is a row's identity
   (`Id: Ord + Clone`), its depth, and whether it has children; a row's
   children are the deeper rows that follow it. The host implements it on
   its own row (`rows::Row`, whose `RowKind` and `RowId` stay in the host);
   `tree::Row<Id, R>` is the plain shape for other hosts and the framework's
   tests.

2. **`TreeState<Id>` keeps what the user did, apart from the rows.** The
   selected identity and the position it was placed at, the expanded set,
   and the window the last frame drew. Every method takes the rows; the
   state never stores them. The cursor is on the copy of its identity
   nearest the kept position. Moves (`step`, `page_down`, `page_up`, `home`,
   `end`) stop at the ends (house rule 2) and a page is the kept window's
   page (house rule 12). Folds (`expand`, `collapse`, `toggle`) act on the
   selected row and leave a row without children alone; `reveal` opens a
   row's ancestors. A jump (`select`) forgets the position.

3. **One rebuild routine.** `begin_rebuild` takes the cursor off the old
   rows, remembering its identity and the position its copy showed at;
   `finish_rebuild` puts it back on the new rows by the first rule that
   applies: the same identity (nearest copy), else the same position
   (clamped), else the host's first-load hint, else the first row. The
   host may change the folds between the two halves, as a first load and
   the Search view's auto-expansion do.

4. **`TreeView` draws the pane from the host's lines.** A bordered pane
   with a title on its border (the host's tab strip), an optional header
   line, an optional footer banner, and a placeholder; the showing rows in
   a window that follows the cursor, with the shared scrollbar gutter. The
   host's producer `Fn(&R, &RowCtx) -> Line<'static>` lays each row out at
   the width it is told; it is called for the rows the window shows only.
   The pane applies the pending look, the ghost look, and the selection
   highlight (active with focus, inactive without) after the producer.
   `render` returns the window it drew; the host keeps it with
   `TreeState::set_viewport` through the frame's `ViewportReport`.

5. **What the host knows, worked out before the frame.** A row's
   `RowState` (ghost, pending, and a `RowMark` the host puts on it, on or
   off) is data the host passes beside the rows, index for index. The App
   works the left pane's out in `App::tree_row_states` (node presence, the
   hand-off, and the pin markers of the native views), so the row renderer
   reads only the answers; a source check keeps the lookups out of
   `ui/left_panel.rs`. A mixed row (Pinned, History) keeps dimming its own
   cells by its tone, so its prefix keeps its colors as before.

6. **Page keys carry nothing.** `Msg::PageDown` and `Msg::PageUp` lose
   their height and `keymap::translate` its height argument; the reducer
   pages through the state's window.

7. **The left pane keeps its look.** No pointer gutter yet: the tree's
   columns (disclosure, kind glyph) are documented in `docs/tui-columns.md`
   and move with the column schema, which brings the gutter.

## Consequences

- Every before-and-after `--snapshot` frame of the six views (771 frames
  with their ANSI styling, at 100x30, 60x20, and 40x10, with stepping,
  ends, paging, folds, regrouping, view switches, and both focus states)
  is byte-identical.
- One behavior changes, for a row listed twice only: a refresh or a
  projection change keeps the cursor on the copy it stood on, and the
  window follows that copy (it followed the first). The per-view slot now
  saves the position with the selection, so it no longer leaks into
  another view.
- The view slot is one value; ADR 0031's per-view state is unchanged in
  what it covers.
- Laying rows out costs the window's rows per frame, not every visible
  row's; the row-state pass still visits every row (the pin marker
  reservation is tree-wide).
- The column schema stories build on the producer seam: they replace the
  producer one row kind at a time and bring the pointer gutter.

## Alternatives Considered

- **A generic row struct the host adopts** (`Row { id, depth, expandable,
  payload }` in place of `rows::Row`). Every `row.kind` in the row
  builders, the renderer, actions, and their tests would become
  `row.payload`, and every depth a `u16`, for no change in meaning; the
  trait keeps the host's row and the framework still offers the struct.
- **A state that owns its rows.** The host rebuilds rows from its
  snapshot on every projection change and reads them from many places
  (actions, preview, quick select); moving them into the state would make
  every reader go through it. The state keeps identities only.
- **A closure for row states called while drawing.** It would keep the
  lookups inside the draw, which is what the renderer is meant to stop
  doing; data worked out before the frame keeps the producer pure.
- **Keep the page height in the message.** The App already keeps the
  window; carrying its height through the keymap is a second source for
  the same number.
- **First-copy retention** (what the code did). It contradicts its own
  documented intent and moves the cursor on refresh in the Mux and PRs
  views, which can list a row twice.
- **Ghosting mixed rows whole in the framework.** Their prefix (disclosure,
  kind glyph, marker) kept its colors; ghosting the whole line would
  change every stopped pin's row.

## Open Questions Answered

- *Does the framework draw the disclosure and kind columns?* Not yet: the
  producer draws them until the column schema declares them.
- *Which copy of a duplicated row is highlighted?* Every copy with the
  selected identity, as before; the window follows the copy the cursor is
  on.
- *Is a draw still a message?* No; the tree's window comes back by value,
  as ADR 0156 decided, and is kept in its state.
