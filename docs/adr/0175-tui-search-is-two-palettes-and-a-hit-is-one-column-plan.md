# ADR 0175: Search Is Two Palettes, And A Hit's Cells Are One Column Plan

## Status

Accepted. Carries out the search instances of the `Palette<R>` control of
[ADR 0174](0174-tui-palettes-one-control-a-query-over-ranked-results.md)
and amends it: quick select keeps its own cursor rule after an edit, and
the palette gains a column plan, progress words, rows the host draws, a
cursor the host can move, and a reserved scrollbar column. The search
semantics of [ADR 0147](0147-session-content-search.md) and
[ADR 0149](0149-two-level-search-current-and-wider-context.md) are
unchanged, including the column order of ADR 0149 item 25, which the plan
states. Amended by
[ADR 0185](0185-tui-a-search-hit-is-one-column-schema-in-the-modal-and-the-search-view.md):
the plan is a column schema (`hit_columns`), `palette::ColumnPlan` is
retired, and the Search view's hit, pane, and pane-hit rows lay the same
columns out. Amended by
[ADR 0196](0196-tui-quick-selects-entity-rows-come-from-a-column-schema.md):
the entity rows the alternatives below left on their own split come from
a column schema too.

## Context

`src/tui/widgets/search.rs` drew quick select (`/`) and content search
(`s`, `Ctrl-/`) itself: its own frame body, query line, title strips,
status row, options pane with its own key table (`SEARCH_OPTION_KEYS`, row
for row the palette's), debounce timer, footer, and a scroll window
re-followed from the top every frame. ADR 0174 extracted the palette with
these parts and listed what the search would inherit; it left three
choices to this step: the cursor rule after an edit, the kept window, and
the scrollbar.

The inventory (`docs/plans/tui-framework/inventory/05-search-help-viewers.md`,
sections 2.7 to 2.9, 6, and 9) found a content hit drawn by a hand-written
grid (`content_columns`: source word, chip, label, count, age, snippet,
with a drop order) that the Search view's hit rows repeat in a grid of
their own, and named a shared plan of columns as the fix.

Facts established while moving the search:

- The search's rows need what the palette cannot hand a `PaletteRow`: the
  host's theme (operator glyph overrides for the kind glyph, the harness
  chip colors) and, for an entity on the wider reach, an outcome word the
  App works out per row (`open_plan`), which scans the view's rows, so
  working it out for every result each frame would cost results times
  rows.
- `consistency_tests/search_quick.rs` (`the_cursor_survives_typing_after_moving`)
  states quick select's rule: typing a character that keeps the matches
  keeps the cursor where it was.
- The content result's rows always left one blank cell before the frame.
  A palette that gives an overflowing list's last column to the scrollbar
  and lays the rows out one cell narrower would move the label share and
  the count's step by a cell whenever a streaming scan's list grew past
  the frame.

## Decision

1. **Two palettes, one per mode.** `SearchOverlayState` holds quick
   select's `Palette<QuickRow, SearchOutcome, SearchKey>` (the Current/All
   strip, a count of the results) or content search's
   `Palette<ContentRow, (), SearchKey>` (the Current/All and
   Sessions/Messages strips, the options pane, the debounce, the status
   row). Both keep `SEARCH_KEYS` and `SEARCH_CONTENT_KEYS` over
   `SearchKey`, which implements `PaletteAction`; regrouping, the promote
   verbs (`Ctrl-F`, `Ctrl-S`), and history paging (`Ctrl-R`, `Ctrl-T`) are
   the adapter's, resolved first with `host_action`. The adapter keeps the
   ranking (`rank`, `EntityIndex::rank`, handed over as results), the
   outcome words, the content kinds, and the `Msg` mapping; `SearchOutcome`
   stays its outcome.

2. **The host draws the search's rows.** `Palette::draw_with` lays the rows
   on screen out with a host closure, which has the host's theme and the
   App's outcome words, so the word is looked up only for the rows shown;
   `Palette::handle_key` routes keys without the overlay trait, so the
   rows need not be `PaletteRow`s. The row layouts live in
   `src/tui/widgets/search/rows.rs`.

3. **A hit's cells are one column plan.** `tuikit::palette::ColumnPlan`
   states a row of several cells: ordered columns (fixed, a bounded share
   of the row, steps by the row's width, or the one fill), the gap after
   each, and the order the others give way in (dropping out, or narrowing
   to a floor) while the fill is short of its minimum, laid out once per
   width (`ColumnLayout`). The content hit's plan (`hit_plan`) is ADR 0149
   item 25 written down: the source word (7), the chip (a chip's footprint
   from the theme), the label (a fifth of the row, 12 to 32), the hit
   count or position (22, 20, or 14 by the row's width), the age (4, at
   the end), and the snippet, which keeps 20 cells; the chip drops first,
   then the source word, then the age, and the label narrows last, to 8,
   shared out over the row with its cursor gutter (`lead`). The Search
   view's hit rows lay the same plan out when they move to the column
   schema.

4. **The rows keep one width.** The search palettes reserve the
   scrollbar's column (`reserve_scrollbar`): the rows end a cell before
   the frame as they always did, and an overflowing list draws the shared
   scrollbar in that column, so no column moves as results stream in. The
   results keep their scroll window between frames (ADR 0174 decision 6).

5. **Quick select keeps its cursor rule.** An edit keeps the cursor's
   position while it is still a result and goes to the first otherwise;
   the adapter restores it with `Palette::select` after the palette's own
   rule. Content search keeps the palette's rule, which was already its
   own.

6. **The status row's progress is the palette's words.**
   `tuikit::palette::Progress` says `34/77 sessions` while a scan runs,
   `34 sessions searched` once it is done, and `· 2 skipped`; the adapter
   keeps the counts, the noun, and the rest of the row.

7. **One options table.** The options pane resolves
   `PALETTE_OPTION_KEYS`; `SEARCH_OPTION_KEYS` and `SearchOptionKey` are
   deleted.

## Consequences

- **Visible changes** (the CSP-743 summary lists them): an overflowing
  result list shows the shared scrollbar in the column its rows leave
  blank, and moving back up a scrolled list no longer drags the list to
  the top. Every other frame is byte-identical, styles included, at
  100x30, 60x20, and 40x10.
- `Ctrl-C` and `q` in the options pane answer the palette's `Dismiss`,
  which the adapter reports as `SearchOutcome::Cancel` (a close): for an
  overlay that is no step on the stack the two settle alike.
- The search tests pass with their expectations unchanged; their helpers
  changed mechanically (drawing borrows the state mutably, two helpers
  live with the tests, the row layouts are imported from `rows`).
- The wider reach's entity rows keep their own split (`entity_columns`):
  the label takes a share of what the outcome word leaves, and a context
  too narrow to read drops and gives its room to the label, two rules the
  plan does not have. The current reach's rows are a label and a snippet
  with no grid. The palette page lists the rules as a seam.
- `PgUp`/`PgDn` are not in the search tables, so the search does not page
  (house rule 12); the palette would page if they were.

## Alternatives Considered

- **One palette that switches mode.** The two modes have different
  tables, strips, counts, options, and debounces; the overlay never
  switches mode while open, so one palette per mode is smaller.
- **Rows as `PaletteRow`s drawn with the framework theme only.** The kind
  glyph would lose the operator's `[tui.theme.icons]` overrides, and the
  outcome words would have to be worked out for every result each frame.
- **The palette's cursor rule for quick select** (ADR 0174 expected it).
  It moves the cursor to the first match on every character, which a
  consistency test of the shipped behavior forbids; the rule stays a
  choice for the operator (the operator review lists how to switch).
- **No reserved column.** Rows would take the last cell while they fit and
  give it back when the list overflows, moving the label share and the
  count's step by a cell mid-scan.
- **Progress inside the palette's status row** (the palette composing the
  host's words with a progress it holds). It would tie the host's nouns
  into palette state; a formatter the host calls keeps the row the host's.
- **The entity rows on the plan.** It needs a share of what the fixed
  columns leave and a fill that drops and hands its room to a neighbor;
  neither is needed by the hit rows, so they wait for a grid that needs
  them.

## Open Questions Answered

- *Which cursor rule does quick select follow?* Its own: the position is
  kept across an edit while it is still a result.
- *Does the search keep the palette's window and scrollbar?* Yes, with the
  scrollbar in a column the rows reserve.
- *Where does the hit column plan live?* In `tuikit::palette`, built by
  the host as `hit_plan`, so a list outside the modal can lay it out.
