# ADR 0188: The Explorer Is A Framework Control Over A Host Source, And The Right Pane's Keys Are One Table

## Status

Accepted as a design (CSP-755) and carried out in full (CSP-789,
CSP-790, CSP-791, CSP-756). Decides the `Explorer` row of
[ADR 0151](0151-tui-framework-control-set-and-house-rules.md)'s control
set, which ADR 0151 staged last behind a design story, and extends
[ADR 0150](0150-tui-framework-workspace-crate.md) with the framework's
`explorer`, `breadcrumb`, and `master_detail` modules, `text::elide`,
and `frame::SectionRule`. The stories that carry it out (CSP-789,
CSP-790, CSP-791, CSP-756) record what they change here. As they land
they narrow [ADR 0074](0074-detail-pane-related-entities-flatten.md)
decision 6 (a refresh keeps the drill and the cursor), apply
[ADR 0085](0085-tui-mvu-architecture.md) contract 1 to the right pane (a
nested `Msg::Explorer(ExplorerKey)` instead of eight messages and a
runtime branch), and bring the right pane under
[ADR 0160](0160-tui-keys-one-model-one-character-rule-one-footer-grammar.md)'s
key model; the index records each amendment then. The design, with the
measurements, the Rust signatures, and the migration, is
`docs/plans/tui-framework/explorer-design.md`.

CSP-789 landed decision 3's refresh rule in the host, ahead of the
framework, and narrowed ADR 0074 decision 6 with it: the app remembers
the left selection the right pane reflects (set when the pane opens or
reloads, after the mirror moves the left pane, and after `Backspace`
restores a frame's selection), and while the selection is still that
row a refresh or a projection change reloads the drilled node in place
and keeps the breadcrumb, the fold, the every-property switch, and the
cursor; when that node is gone the pane opens on the selection's node.
A jump to a row through `set_selection` (a pick, quick select's
`here`, a regroup, a follow) opens the pane again even onto the row
already selected, so the adapter calls `open` there, not `refresh`.
`Backspace` to a frame whose node is gone still takes a press of its
own; the framework's `back` rule changes that.

CSP-790 landed decision 6: `tuikit::text::elide` (with `Piece`,
`MarkAt`, `Elided`, and `Item`), `marks::CRUMB` and
`marks::SPACED_CRUMB`, and `tuikit::breadcrumb::Breadcrumb` over `Hop`s
built with `Hop::new(tag).glyph(..).suffix(..)`. `Breadcrumb::new`
takes the theme and draws in its `inert` unless `.style` replaces it.
A suffix shows when two hops have the same tag (the tag the title
draws, so two kinds with one name now carry it too). The right pane's
title gives the chain the cells its corners and other parts leave (it
had been handed the pane's whole width, so the corner clipped a full
title's pad and sometimes the depth's digit), and its trailing pad
gives way before any hop.

CSP-791 landed decision 7, byte for byte:
`tuikit::master_detail::StackedMasterDetail` (`min_list` 3 and
`min_detail` 6 by default; `layout_width`, `areas`, and `render` over
the host's `GridLines`, a divider line, and the window it kept,
returning a `Stacked` with the window and the three areas) and
`tuikit::frame::SectionRule` (`new(label)`, `.summary(..)`,
`.summary_first()`, `.line(width, &theme)`). The split is hand
arithmetic instead of ratatui's solver, with the solver's result on an
area under the list's minimum and the divider (the list's first row,
then the divider's, then the rest of the list). An empty summary shows
nothing, where the host drew two spaces. The rule does not give way
below its own width, as before (the `Related` rule wraps onto a second
row at 40 columns); CSP-794 follows that up.

CSP-756 landed decisions 2 to 5. `tuikit::explorer` holds
`ExplorerModel`, `ExplorerSource`, `Zone` and `Entry` (public fields,
with `Zone::new(..).heading(..).lands()` and `Entry::new(..).under(..)`
builders), `FrameView`, `Frame`, `Explorer<M>`, the ten-variant
`Outcome`, `Refusal`, `Refreshed`, `ExplorerKey`, `EXPLORER_KEYS`, and
`hint`; `keys::exact` binds `Enter` with no modifier, and
`help::sections_described` generates a page from a framework table in
the host's words. Conspectus adapts it in `explorer::source` (`Graph`,
`GraphSource`), keeps `App.explorer: Option<Explorer<Graph>>` and lends
it out with a source built from its other fields for each call, carries
out each outcome in one function, and resolves `RIGHT_PANE_KEYS` (the
explorer's table) before its keymap with a focus guard. Departures from
the design, each recorded in the operator review: `seek`'s test takes
the content beside the entry, since it cannot borrow the explorer it is
moving; a page moves by the rows the zone showed (`keep_window` takes
the window only), not by a count of entries, which the layout does not
report; a `Back` whose every frame is gone drops them all and keeps what
is shown; a drill and a `Back` keep the zone's window, and the host
carries it into an explorer it opens afresh, as the App's field did, so
the frames stayed byte-identical; `Action::OpenValueModal` goes too
(`o` sends the explorer's `Inspect` on either pane); the global
`Backspace` is a new `Msg::NavBack`, which says there is no reveal to
return from where it used to pop the right pane's drill; and the status
hint takes `Enter`'s verb from the cursor's row (`Explorer::footer`).
The stop rule did not apply: the 26 state-machine tests kept their
assertions (the copy tests read `Effect::Copy`), and 1015 right-pane
frames were byte-identical after the adoption and before the key
changes.

## Context

The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/07-chrome-and-explorer.md`,
section 4) found the right pane's explorer to be a coherent control
written as `impl App`: a flat cursor over zones of rows with stable
keys, one collapsible section, a switch between a node's summary and
all its fields, a drill stack whose frames save that state, and a
`Backspace` that arms before it leaves the pane. Measured again at
`a5af2807`:

- **Most of the code is domain.** 74% of `explorer.rs` (2,109 lines)
  builds the view model from the graph; the builders alone are 47.6%.
  The control flow sits in `app/explorer_nav.rs` (355 lines, about 190
  of them generic), `ExplorerState` and the cursor-row helpers in
  `app.rs`, the layout and rule in `ui/right_panel.rs`, and the remap in
  `keymap.rs`.
- **It reaches into the App.** The transitions read and write seven
  fields besides the explorer (`explorer_window`, `explorer_back_armed`,
  `focus`, `edge_meta_visible`, `status_message`, `tree_state`, and,
  read only, the snapshot, the config, the tree, the Preview's region,
  and the reveal history), so no test of the state machine runs without
  an `App` and a `GraphSnapshot`; 26 such tests exist.
- **Keys go three ways.** `remap_for_focus` rewrites the left pane's
  actions for the right pane; `Enter` becomes `Action::ExplorerEnter`,
  which the live loop, the fixture loop, and the snapshot driver each
  branch on to copy before falling back to a message; three hand-written
  texts describe the keys (the help section's `e` is stale).
- **Defects.** The 30-second refresh throws a drill away when the
  drilled node has no row in the active view; `Backspace` on the left
  pane pops the right pane's drill when there is no reveal history;
  `Backspace` to a node that has gone takes a press of its own and no
  test covers it; page keys move one row; `Esc` does nothing.

`tui-tree-widget` (CSP-435) does not fit: the explorer is a list with a
section, not a tree.

## Decision

1. **Go, in steps.** The explorer's control flow moves to the
   framework. A defect fix (CSP-789), `Breadcrumb` with its fit rule
   (CSP-790), and the layout with its rule (CSP-791) land first and on
   their own; the core, its adoption, and the key table are CSP-756.
   Each step leaves `main` releasable. If the adoption cannot keep the
   state-machine tests and the right pane's frames unchanged with no
   more than the lending pattern of ADR 0161, the core stays in
   Conspectus and the rest still extracts.

2. **The host supplies a source.** `tuikit::explorer::ExplorerModel`
   names the kinds of value the explorer keeps (an `Id` it can show, a
   row `Key` stable across rebuilds, the host's own `Row`, the
   `Content` for one thing, and what the host `Saved` with a frame).
   `ExplorerSource` is built from the host's data for each call: `load`
   an id, the `id` of a content, its `zones` for a view, where a link
   row `follow`s, the breadcrumb `tag`, what to `save` with a frame,
   and the `grid` its rows draw in. The explorer keeps owned values
   only, so it lives in the host's model.

3. **The explorer keeps the state and its rules.** `Explorer<M>` holds
   the content shown, the cursor (index and key), the open sections and
   the every-property switch (a `FrameView`), the drill `Frame`s, the
   arm, and the zone's window. After every rebuild the cursor goes back
   to the same key, else the first row of the first zone that may take
   it (`Zone::lands`), else the first row. `refresh` reloads what is
   shown and keeps the drill; the host calls it when its own selection
   did not change and opens the explorer again when it did. `Back`
   restores a frame's view and cursor and skips frames whose thing is
   gone. `Back` with no frames arms; again, leaves.

4. **Keys answer with an `Outcome`.** `handle(ExplorerKey, &source)`
   does the explorer's part and answers `None`, `Copy`, `Inspect`,
   `Toggle`, `Drill`, `Follow` (a link whose far end does not load),
   `Back` (with the frame's saved value), `Armed`, `Leave`, or
   `Refused(Refusal)`. The property grid's intents feed it: a row's
   `Copy` and `Inspect` pass through, `Toggle` folds, `Follow` drills.
   The host does every effect (the copy is `Effect::Copy`, the inspect
   opens the value modal, a drill mirrors the left pane, `Leave` moves
   focus) and words every hint and refusal. `Action::ExplorerEnter` and
   the runtime's copy branch go.

5. **The right pane's keys are one table.** `tuikit::explorer::EXPLORER_KEYS`
   binds the moves, page keys, ends, `Enter` (exactly, so `Shift-Enter`
   and `Alt-Enter` still reveal), `e` and `h`/`l` and the arrows for
   sections, `o`, `F`, `Backspace`, and `Esc` as `Back`. Conspectus
   resolves it as `RIGHT_PANE_KEYS` before its global keymap when the
   right pane has focus and no modal is open, drops the global table's
   left-tree actions there, and generates the help section and the
   status hint from it. The left pane's meanings of `F`, `Enter`, and
   `Backspace` stay in the global table until the scoped keymap
   (CSP-760) takes both as scopes of one table. The left pane's
   `Backspace` stops popping the right pane's drill.

6. **One fit rule for pieces of a line.** `tuikit::text::elide` keeps
   pieces by rank, marks each run that gave way (in its gap, or once at
   the end), and cuts the last kept piece when nothing else can go.
   `tuikit::breadcrumb::Breadcrumb` draws the drill chain on it (the
   last hop always, then the first, then the newest middle hops), and
   `TabBar` and `PriorityLine` (CSP-758) use it with the mark at the
   end. The title's 8-cell floor goes.

7. **The layout and its rule are framework controls.**
   `tuikit::master_detail::StackedMasterDetail` sizes a list zone to its
   wrapped rows (at least three), keeps six rows for the detail when the
   area allows, draws the divider and the list's window with the shared
   gutter, and returns the window and the detail area.
   `tuikit::frame::SectionRule` is the zone rule (`chip_divider_line`).

8. **Conspectus keeps the domain.** The node builders, `NodeView`, the
   row keys, the verbs, the tones' roles, kind glyphs, the Preview, the
   search hit block, the mirror, the edge meta flag, the title's parts,
   and every word the operator reads.

## Consequences

- Visible, as the stories land: a drill survives a refresh; the
  breadcrumb keeps more hops and cuts its last one instead of being
  clipped; page keys page the right pane; `h`/`l` and the arrows fold
  its section; `Esc` goes back; the left pane's `Backspace` leaves the
  right pane alone; `Alt-j` and `Alt-k` stop moving the explorer; the
  help section and the status hint are generated, so their wording
  changes and the stale `e` text goes. Each story lists its changes.
- The explorer is testable without the App: the framework's tests drive
  it over a toy source, and the host's 26 state-machine tests stay as
  integration tests of the adapter, renamed. The builder tests in
  `explorer_tests.rs` stay as they are.
- The snapshot driver's skipped copy posts the copy's toast, so the
  value modal's and Messages' copies show it in `--snapshot` as `Enter`
  does.
- `TabBar` (CSP-758) waits for CSP-790 so the strip and the breadcrumb
  share their rule.

## Alternatives Considered

- **Keep the explorer in Conspectus** (no-go). Lower risk, but the
  `ExplorerEnter` branch, the three key texts, the arm on `App`, and the
  untestable state stay; `Breadcrumb` and the layout would still
  extract. Kept as the fallback of decision 1.
- **Content kept outside the explorer**, as `TreeState` keeps no rows.
  A drill and a `Back` change the content, so the host would do half of
  each move and the drill's rules would not be testable alone.
- **One source trait with the host's data as its type parameter.** A
  source that borrows the snapshot has a lifetime, which would then be
  in the `App` field's type; splitting the stored types (the model) from
  the data access (the source) keeps the field plain.
- **The explorer as an overlay** on the modal stack. It is a pane: it is
  drawn whether or not it has focus and must not own every key.
- **Reuse `TreeState` for the cursor.** Its reseat falls back to the
  same position, which would drop a lost link cursor into the fields;
  the explorer's rows are zones, not a tree.
- **Keep one `Msg` per key** and the runtime's copy branch. It keeps the
  three loops' branches and the copy outside the effect executor.
- **A static `Enter` verb only** in the status hint. Simpler, but
  `Enter` copies, follows, or folds by row; the hint can say which.
- **Keep today's breadcrumb ladder** in `Breadcrumb`. Byte for byte, but
  it would be a fourth fit rule beside the strip's and the bar's.

## Open Questions Answered

- *Go or no-go?* Go, with a stop rule (decision 1).
- *What does the host supply?* A model of five types and a source of
  seven calls (decision 2).
- *Who owns the effects?* The host, from the outcome (decision 4).
- *Which table declares the keys that change meaning by focus?*
  `EXPLORER_KEYS`, as `RIGHT_PANE_KEYS`, until the scoped keymap makes
  both meanings scopes of one table (decision 5).
- *How do the breadcrumb and the tab strip share code?* One
  `text::elide` rule, ranked differently (decision 6).
- *Does `tui-tree-widget` fit?* No; CSP-435's premise is gone.
