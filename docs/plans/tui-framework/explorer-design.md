# Explorer Design: The Zoned Cursor And Drill Stack Without `App`

Design spike CSP-755 for milestone m-25. It decides whether the right
pane's explorer (a cursor over zones of rows, a drill stack, an armed
`Backspace`) becomes a framework control, specifies the control if so,
and cuts the move into steps. CSP-756 and the stories it now depends on
carry it out. The decision is recorded in
[ADR 0188](../../adr/0188-tui-the-explorer-is-a-framework-control-over-a-host-source.md).

Measured at commit `a5af2807` (after CSP-754). Line numbers drift as the
steps land; re-find the code.

**Verdict: GO**, in seven steps, the first three of them small and
useful whatever happens to the rest (section 8). In short:

- `tuikit::explorer` keeps the state: the content shown, the cursor (by
  index and by the row's stable key), the open sections, the
  "every property" switch, the drill frames, the armed `Back`, and the
  zone's scroll window. It reads the host's data only through an
  `ExplorerSource` the host builds for each call, and it answers every
  key with an `Outcome` instead of reaching into `App`.
- `tuikit::breadcrumb` draws the drill chain on one fit rule
  (`tuikit::text::elide`) that `TabBar` and `PriorityLine` (CSP-758)
  share. `tuikit::master_detail::StackedMasterDetail` and
  `tuikit::frame::SectionRule` lay out and rule the pane.
- The right pane's keys are one table, `tuikit::explorer::EXPLORER_KEYS`,
  which Conspectus resolves (as `RIGHT_PANE_KEYS`) before its global
  keymap when the right pane has focus.
- Conspectus keeps the node builders, `NodeView`, the row keys, the
  verbs, the Preview, the left-pane mirror, and every word the operator
  reads.
- One defect found while measuring is fixed first (CSP-789): the
  30-second refresh throws a drill away.

## 1. The code today, measured

Lines are physical lines; "code" excludes blank and comment lines. The
ranges were measured with a throwaway counter over the files at
`a5af2807`.

### 1.0 By responsibility, across the files

Summed from the per-file tables below (doc comments included; tests
excluded):

| Responsibility | Lines | Where | Moves to the framework |
| --- | --- | --- | --- |
| Node builders and other domain content | about 1,900 | `explorer.rs` (view model, verbs, relationship types, builders, `row_preview`: 1,628), `explorer_nav.rs` (`recompute_explorer`, the mirror, edge meta: 65), `right_panel.rs` (kind label, Preview, hit block, empty text: 215) | none; plus `explorer/grid.rs` (262), already the property grid's adapter |
| Zone and cursor model | about 495 | `explorer.rs` (row identity, zone walk: 251), `ExplorerState` in `app.rs` (88), `explorer_nav.rs` (cursor, fold, full, activate: 101; load and reseat: 55) | about 210 (the state, the moves, the reseat, the fold rule); the keys and the zone walk stay as the source |
| Drill and back stack | about 130 | `explorer.rs` (`BreadcrumbHop`, `LinkRef`: 40), `explorer_nav.rs` (drill: 55, pop and restore: 35) | about 90; the mirror and the selection's restore stay |
| Breadcrumb | about 200 | `explorer.rs` (141), `right_panel_title`'s budget (61) | about 110 (the fit and its docs, the budget); the hops' glyphs stay |
| Armed back | about 46 | `explorer_nav.rs` (29), the field and its clearing rule in `app.rs` (17) | about 35; the host keeps one `disarm()` call |
| Key handling | about 200 | `remap_for_focus` (64), `Action` docs (31), `keybindings.rs` rows (30), `runtime.rs` (26), `snapshot.rs` (3), the help section (27), the status hint (6), the reducer arms (15) | about 115 (the remap's right arm, the runtime and snapshot branches, the two hand-written texts, all but one reducer arm) |
| Rendering glue | about 285 | `draw_right_panel` (102), `chip_divider_line` (57), `render_explorer_lines` and `related_rule` (55), the copy and inspect helpers in `app.rs` (72) | about 190 (the layout, the rule, the lines, and most of the helpers, which shrink to the outcome mapping) |

About 750 lines of host code are replaced by framework code, out of
about 3,400 lines of explorer code in the host (22%), in line with the
inventory's 20 to 25%.

### 1.1 `src/tui/explorer.rs` (2,109 lines, re-measured; 2,156 at `ca668a3`)

| Lines | Code | Share | Responsibility | Domain or generic | Goes to |
| --- | --- | --- | --- | --- | --- |
| 1-46 | 18 | 2.2% | module docs, imports, `mod grid` | — | stays |
| 47-105 | 46 | 2.8% | builder entry: `ExplorerInputs`, `build_node_view` | domain | stays; called by `GraphSource::load` |
| 106-272 | 98 | 7.9% | view model: `NodeView`, `CoreField`, `FieldValue` | domain | stays (`NodeView` is the content) |
| 273-394 | 86 | 5.8% | `Direction`, group sort, `directional_verb` | domain | stays |
| 395-555 | 89 | 7.6% | relationship model types | domain | stays |
| 556-673 | 72 | 5.6% | row identity: `ExplorerRowKey`, `NeighborRowKey`, `ExplorerRow`, `key()` | domain-shaped keys over a generic role | stays as the host's `Key` and `Row` |
| 674-806 | 95 | 6.3% | zone composition: `flat_rows`, `has_other_rows`, counts, `fields` | the walk is generic, the groups are domain | becomes `GraphSource::zones`; the fold filtering rule moves |
| 807-846 | 15 | 1.9% | drill frame `BreadcrumbHop`; dead `LinkRef` | generic frame, domain fields | becomes `explorer::Frame`; `LinkRef` deleted |
| 847-913 | 58 | 3.2% | row actions: `drill_target`, `row_preview` | domain | `drill_target` becomes `GraphSource::follow`; `row_preview` stays |
| 914-939 | 11 | 1.2% | breadcrumb doc and entry | generic | `tuikit::breadcrumb` |
| 940-984 | 38 | 2.1% | breadcrumb hops from nodes (kind glyph, tag, id suffix) | domain | stays as the hop adapter |
| 985-1054 | 63 | 3.3% | breadcrumb fit ladder | generic | `tuikit::text::elide` and `tuikit::breadcrumb` |
| 1055-1102 | 35 | 2.3% | `RowPreview`, `EdgeStateLabel` | domain | stays |
| 1103-2106 | 904 | 47.6% | builder internals: per-kind fields, `build_explorer`, `finalize_group` | domain | stays |
| 2107-2109 | 3 | 0.1% | tests module | — | stays |

The inventory's "about 48% builders, about 13% domain-free" holds: the
builder internals alone are 47.6%, and the domain view model, verbs,
and relationship types bring the domain share to about 74%. What is
generic in shape (row identity, the zone walk, the frame, the
breadcrumb renderer) is 432 lines (20.5%), but most of it carries
domain types, so only the breadcrumb's fit and docs (96 lines), the
zone walk's fold rule, and the dead `LinkRef` (about 120 lines, 6%)
leave the file. It shrinks to about 1,990 lines.

`src/tui/explorer/grid.rs` (262 lines, CSP-754) maps rows onto the
property grid. It is already an adapter and stays, gaining the zone
assembly.

### 1.2 `src/tui/app/explorer_nav.rs` (355 lines)

| Lines | Code | Responsibility | Goes to |
| --- | --- | --- | --- |
| 7-42 | 34 | `recompute_explorer`: `RowId` to `NodeId` | stays (host) |
| 44-98 | 47 | `recompute_explorer_for`: load, keep state when the focused node is the same, reseat | load stays; keep-and-reseat becomes `Explorer::refresh`; dead `strip_relationships` deleted |
| 100-128 | 24 | cursor step, Home and End | `Explorer::handle` |
| 130-154 | 19 | toggle the `Other` zone, with a hint elsewhere | `Explorer::handle` + `Refusal::NotInSection` worded by the host |
| 156-163 | 8 | toggle edge meta | stays (global `E`) |
| 165-184 | 17 | toggle Expanded Node Detail, hint when no extras | `Explorer::handle` + `Refusal::NothingMore` |
| 186-212 | 27 | activate: dispatch on the row's role | `Explorer::handle` (by the grid row's intent) |
| 214-268 | 46 | drill: build the hop, load, push, reset the Preview, mirror | push and load: `Explorer::drill_to`; Preview and mirror: host on `Outcome::Drill` |
| 270-290 | 13 | mirror the left pane onto the drilled node | stays (host) |
| 292-320 | 23 | back with no frames: arm, then shift focus; hint on the left | `Explorer::back` (`Armed`, `Leave`); the left pane's hint stays host |
| 321-355 | 31 | back: pop, load, restore folds and cursor, restore the left selection | pop, load, restore: `Explorer::back`; left selection: host on `Outcome::Back` |

About 190 of the 355 lines are control flow the framework replaces; the
rest (about 150) stays as the adapter: resolving the left selection to a
node, the mirror, the left selection's restore, the edge meta flag, and
the outcome's words.

### 1.3 Explorer code in `src/tui/app.rs` (1,933 lines; 230 lines are the explorer's)

| Lines | Code | Responsibility | Goes to |
| --- | --- | --- | --- |
| 135-155 | 4 | fields `explorer`, `focus`, `edge_meta_visible`, `explorer_back_armed` | see section 1.6 |
| 235-241 | 1 | field `explorer_window` | moves into `Explorer` |
| 397-422 | 8 | `ExplorerState` | replaced by `Explorer<Graph>` |
| 423-484 | 39 | `new`, `rows`, `selected_row`, `reseat_cursor` | replaced by `Explorer::{open, entries, cursor_entry}` and the reseat rule |
| 653-724 | 48 | cursor row, copy target, open the value modal | `Outcome::Copy` and `Outcome::Inspect`; the host keeps the effect and the modal |
| 954-969 | 9 | accessors | stay, typed by the framework |
| 1224-1234 | 4 | the arm-clearing rule in `reduce` | stays as one `disarm()` call |
| 1278-1292 | 15 | reducer arms for eight explorer `Msg`s | one `Msg::Explorer(ExplorerKey)` arm |

### 1.4 `src/tui/ui/right_panel.rs` (496 lines, re-measured; 921 at `ca668a3`)

| Lines | Code | Responsibility | Goes to |
| --- | --- | --- | --- |
| 15-116 | 74 | `draw_right_panel`: layout width, wrap, zone heights, split, window, gutter, divider, Preview call | layout: `StackedMasterDetail` (about 55 lines); the hit block and Preview call stay |
| 118-178 | 35 | title: kind glyph and label, breadcrumb budget, back mark, depth | budget and fit: `Breadcrumb`; the title's parts stay |
| 180-202 | 20 | kind label | stays |
| 204-260 | 45 | `chip_divider_line`, `ChipAnchor` | `tuikit::frame::SectionRule` |
| 262-316 | 45 | `render_explorer_lines`, `related_rule` | `Explorer::lines`; the rule's summary words stay in the zone heading |
| 318-452 | 117 | the Preview for a Related row | stays (domain; ADR 0107, 0112) |
| 454-496 | 39 | empty text, search hit block | stays |

### 1.5 Key handling

| Where | Lines | What | Goes to |
| --- | --- | --- | --- |
| `keymap.rs` `remap_for_focus` | 52 | right focus: `j`/`k`/arrows and the page keys to one step, `h`/`l`/arrows dropped, Home/End, Enter to `ExplorerEnter`, `F` to `ExplorerToggleFullDetail`; left focus: `e` dropped | right arm replaced by `RIGHT_PANE_KEYS` and a focus guard (section 7.3) |
| `keymap.rs` `Action` | 31 | `OpenValueModal`, `ExplorerEnter`, `CopySessionId` and their docs | `ExplorerEnter` deleted; the others stay |
| `keybindings.rs` | 30 | rows for `e`, `E`, `F`, `Enter`, `Backspace` | `e` row deleted (only the right table binds it); the rest stay as the left pane's meanings |
| `runtime.rs` | 26 | `explorer_enter_action`, `explorer_enter` with an injected clipboard, two dispatch arms | deleted; a copy is `Effect::Copy` |
| `snapshot.rs` | 3 | `ExplorerEnter` with a no-op clipboard | deleted; the snapshot's copy skip posts the copy's toast |
| `widgets/help.rs` | 27 | hand-written "Detail-pane graph explorer (right focus)" section; its `e` text is stale | generated from `RIGHT_PANE_KEYS` |
| `ui/header.rs` | 6 | hand-written right-focus hint `j/k cursor · Enter drill/expand · e group · Backspace back` | generated from the table's footer |

### 1.6 The `App` fields the state machine uses, and where each goes

The inventory counted seven (`explorer_scroll`, `explorer_back_armed`,
`focus`, `edge_meta_visible`, `status_message`, `selection`, `detail`).
Since then CSP-724 turned `explorer_scroll` into `explorer_window`,
CSP-750 turned `selection` into `tree_state`, and CSP-710 deleted
`detail`. Counted again over `explorer_nav.rs`, the `ExplorerState`
block, and the cursor-row helpers:

| Field (today) | Inventory name | Used for | Destination |
| --- | --- | --- | --- |
| `explorer: Option<ExplorerState>` | (the state itself) | everything | stays an `App` field, typed `Option<tuikit::explorer::Explorer<explorer::Graph>>`; `NodeView` is its content |
| `explorer_window: ScrollWindow` | `explorer_scroll` | the zone's window across frames | moves into `Explorer` (`window`, `keep_window`); the `App` field goes; `ViewportReport.explorer` stays and feeds it |
| `explorer_back_armed: bool` | `explorer_back_armed` | the two-press leave | moves into `Explorer` (`is_armed`, `disarm`); `reduce` keeps calling `disarm` on every other message |
| `focus: Focus` | `focus` | arm vs hint, the shift to the left pane, the remap | stays; `Outcome::Leave` sets it; the right table is resolved only with right focus |
| `edge_meta_visible: bool` | `edge_meta_visible` | link rows' meta | stays (global `E`); copied into `GraphSource` for `zones` |
| `status_message: Option<Feedback>` | `status_message` | 16 writes: hints and refusals | stays; one host function words each `Outcome` |
| `tree_state: TreeState<RowId>` | `selection` | the node the pane follows, the frame's saved selection, the mirror, the restore | stays; `GraphSource::save` reads it; the host writes it on `Drill` and `Back` |
| (`detail`) | `detail` | — | gone (CSP-710) |

Read but not counted by the inventory: `handle` and `config` (the
snapshot and home directory, into `GraphSource`), `tree` (the mirror's
row lookup, host), `preview` (reset on drill and back, host), and
`nav_back` (the left pane's `Backspace`, host). CSP-789 adds one host
field, the left selection the pane reflects.

### 1.7 Behavior found while measuring

Checked with throwaway probes (section 10), not committed:

1. **A refresh throws a drill away.** `recompute_explorer` keeps the
   pane's state only when the left selection names the focused node.
   After a drill into a neighbor with no row in the active view (from a
   session in the Sessions view to its mux, checkout, repo, or process),
   the next refresh (every 30 seconds in the live TUI) puts the pane
   back on the session with an empty breadcrumb. A reducer probe
   reproduced it. Filed as CSP-789, ahead of the extraction.
2. **`Backspace` on the left pane pops the right pane's drill** when
   there is no reveal history: the reducer falls through to
   `explorer_back`, which pops a hop whatever the focus. The status bar
   never advertises it (it shows `Backspace back` only with reveal
   history). Probe: drill, `Tab`, `Backspace`, and the title loses its
   breadcrumb. The design gives the left pane's `Backspace` to the left
   pane only (section 7.1).
3. **`Backspace` to a node that has gone** drops that hop, warns, and
   stays where it was; the next press goes on to the older frames. No
   test covers it. CSP-789 pins it, and the framework's `back` rule
   skips a gone frame in the same press.
4. **`o` on the left pane** opens the right pane's cursor row's value,
   whichever pane has focus. Kept as is (section 7.1); the scoped keymap
   (CSP-760) may narrow it.
5. **Page keys move one row** on the right pane and **`Esc` does
   nothing** there (probes `pgdn-right`, `esc-right`), as the inventory
   said.
6. **Alt chords of `j` and `k` move the explorer** because the global
   table matches characters with any modifier but Control; the
   framework's one character rule (ADR 0160) does not. Section 7.3.
7. **Dead code in the state machine**: `strip_relationships` is always
   `false`; `LinkRef` has no reader.

## 2. The generic explorer

### 2.1 Modules

| Module | Holds | Story |
| --- | --- | --- |
| `tuikit::text::elide` (re-exported from `tuikit::text`) | `Piece`, `MarkAt`, `Elided`, `elide`: pieces of a line that give way by rank | CSP-790 |
| `tuikit::breadcrumb` | `Hop`, `Breadcrumb` | CSP-790 |
| `tuikit::frame::SectionRule` | the zone rule: muted rule, label chip, optional summary | CSP-791 |
| `tuikit::master_detail` | `StackedMasterDetail`, `Stacked` | CSP-791 |
| `tuikit::explorer` | `ExplorerModel`, `ExplorerSource`, `Explorer`, `Zone`, `Entry`, `FrameView`, `Frame`, `Outcome`, `Refusal`, `Refreshed`, `ExplorerKey`, `EXPLORER_KEYS` | CSP-756 |

The explorer builds on `property_grid` (rows and intents), `selection`
(the cursor look and `Ends::Clamp`), `scroll` (the window), and `keys`
(the table). It adds no dependency.

### 2.2 What the host supplies: `ExplorerModel` and `ExplorerSource`

The explorer keeps owned values only, so it can live in the host's
model; the host's data reaches it through a source built for each call.
A borrowing source with a lifetime cannot be the explorer's type
parameter (the `App` field would need one), so the stored types and the
data access are two traits:

```rust
use std::fmt::Debug;

use crate::frame::SectionRule;
use crate::property_grid::{PropertyGrid, Row};

/// The kinds of value an explorer keeps, named once by the host.
pub trait ExplorerModel {
    /// A thing the explorer can show. A drill frame keeps one.
    type Id: Clone + PartialEq + Debug;
    /// A row's identity across rebuilds: what the cursor keeps, what a
    /// frame restores, and how an open section is named (by its
    /// header's key). Two rows of one content never share a key.
    type Key: Clone + PartialEq + Debug;
    /// The host's own row, handed back with the cursor's entry, so the
    /// host can read what it built (a field's typed value, a link's
    /// group) without looking the row up again.
    type Row: Clone + Debug;
    /// What the explorer shows for one thing, built by the source.
    type Content;
    /// What the host keeps with a frame it drills away from, handed
    /// back when `Back` returns to it.
    type Saved: Clone + PartialEq + Debug;
}

/// What the host supplies. A source is built for each call from the
/// host's data; the explorer keeps nothing it borrows.
pub trait ExplorerSource {
    type Model: ExplorerModel;

    /// The content for `id`, or `None` when the data no longer has it.
    fn load(&self, id: &IdOf<Self>) -> Option<ContentOf<Self>>;

    /// The identity `content` shows.
    fn id(&self, content: &ContentOf<Self>) -> IdOf<Self>;

    /// The rows of `content`, zone by zone, as `view` asks: a closed
    /// section lists its header and none of its rows, and `view.full`
    /// lists every property instead of the summary. Called for every
    /// key and every frame; nothing is cached.
    fn zones(
        &self,
        content: &ContentOf<Self>,
        view: &FrameView<KeyOf<Self>>,
    ) -> Vec<Zone<Self::Model>>;

    /// Where `entry`, a link row, leads; `None` for a link with no far
    /// end the host can name.
    fn follow(&self, content: &ContentOf<Self>, entry: &Entry<Self::Model>)
        -> Option<IdOf<Self>>;

    /// The tag a breadcrumb gives `content`. Its frame keeps it.
    fn tag(&self, content: &ContentOf<Self>) -> String;

    /// What the host keeps with the frame being drilled away from.
    fn save(&self) -> SavedOf<Self>;

    /// An empty grid laid out the host's way (label and verb columns,
    /// the inspect key its hint names, its tones). The explorer draws
    /// its rows in it.
    fn grid(&self) -> PropertyGrid {
        PropertyGrid::new()
    }
}

pub type IdOf<S> = <<S as ExplorerSource>::Model as ExplorerModel>::Id;
pub type KeyOf<S> = <<S as ExplorerSource>::Model as ExplorerModel>::Key;
pub type ContentOf<S> = <<S as ExplorerSource>::Model as ExplorerModel>::Content;
pub type SavedOf<S> = <<S as ExplorerSource>::Model as ExplorerModel>::Saved;
```

What each piece the task asked for is:

| The host supplies | Through |
| --- | --- |
| zones and rows with stable keys | `zones` returns `Zone`s of `Entry`s, each with its `Key` and its grid `Row` |
| row actions | the grid row's intents (`activate`, `inspect`), plus `follow` for a link's far end |
| the reseating rule after a rebuild | each zone's `lands` flag (section 2.5, rule 2) |
| breadcrumb hop data | `tag` (kept with the frame) and, at draw time, the host's own glyph for each frame's `Id` |
| the frame's saved context | `save` (the left pane's selection) |

### 2.3 Zones, entries, the view, and frames

```rust
/// A run of rows under an optional rule.
pub struct Zone<M: ExplorerModel> {
    /// A rule over the zone (a label and a summary) that the cursor
    /// never visits.
    pub heading: Option<SectionRule>,
    /// Whether a cursor whose row is gone after a rebuild may land on
    /// this zone's first row.
    pub lands: bool,
    pub entries: Vec<Entry<M>>,
}

/// One row: its key, how it draws and what it asks for, and the host's row.
#[derive(Clone, Debug)]
pub struct Entry<M: ExplorerModel> {
    pub key: M::Key,
    /// A property, link, or section header row of the property grid.
    pub row: Row,
    /// The key of the section header this row sits under, so the fold
    /// keys can close the section from inside it.
    pub under: Option<M::Key>,
    pub host: M::Row,
}

/// How a frame shows its content.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FrameView<K> {
    /// The section headers that are open, by key.
    pub open: Vec<K>,
    /// Whether property zones list every property rather than their
    /// summary.
    pub full: bool,
}

/// A frame of drill history: the thing shown before a drill, how it was
/// shown, and where its cursor stood.
#[derive(Clone, Debug)]
pub struct Frame<M: ExplorerModel> {
    pub id: M::Id,
    pub tag: String,
    pub view: FrameView<M::Key>,
    pub cursor: Option<M::Key>,
    pub saved: M::Saved,
}
```

`Frame` replaces `BreadcrumbHop`: `focused` is `id`, `short_label` is
`tag`, `cursor_key` is `cursor`, `other_expanded` and
`full_detail_expanded` are `view`, `left_pane_selection` is `saved`.

The `Clone`, `Debug`, and `PartialEq` impls of the types generic over
`M` are written by hand over `M`'s associated types, so the host's
model (a unit struct) needs no derives of its own.

### 2.4 The state

```rust
pub struct Explorer<M: ExplorerModel> {
    content: M::Content,
    id: M::Id,
    tag: String,
    view: FrameView<M::Key>,
    /// The cursor's place among the entries and its row's key.
    cursor: Option<(usize, M::Key)>,
    /// Drill history, oldest first. The frame shown is not in it.
    frames: Vec<Frame<M>>,
    armed: bool,
    window: ScrollWindow,
    /// The entries the last frame showed, which a page key moves by.
    page: usize,
}

impl<M: ExplorerModel> Explorer<M> {
    // Opening and reading.
    pub fn open<S: ExplorerSource<Model = M>>(id: &M::Id, source: &S) -> Option<Self>;
    pub fn content(&self) -> &M::Content;
    pub fn id(&self) -> &M::Id;
    pub fn view(&self) -> &FrameView<M::Key>;
    pub fn cursor(&self) -> Option<usize>;
    pub fn cursor_key(&self) -> Option<&M::Key>;
    pub fn frames(&self) -> &[Frame<M>];
    pub fn is_armed(&self) -> bool;
    pub fn entries<S: ExplorerSource<Model = M>>(&self, source: &S) -> Vec<Entry<M>>;
    pub fn cursor_entry<S: ExplorerSource<Model = M>>(&self, source: &S) -> Option<Entry<M>>;

    // Keys, and the moves a host makes without a key.
    pub fn handle<S: ExplorerSource<Model = M>>(&mut self, key: ExplorerKey, source: &S) -> Outcome<M>;
    pub fn drill_to<S: ExplorerSource<Model = M>>(&mut self, id: M::Id, source: &S) -> Outcome<M>;
    pub fn back<S: ExplorerSource<Model = M>>(&mut self, source: &S) -> Outcome<M>;
    /// Put the cursor on the first entry `find` accepts, opening a
    /// closed section that holds one. Says whether it found one.
    pub fn seek<S: ExplorerSource<Model = M>>(&mut self, source: &S, find: impl Fn(&Entry<M>) -> bool) -> bool;
    pub fn disarm(&mut self);

    // Rebuilds and frames.
    pub fn refresh<S: ExplorerSource<Model = M>>(&mut self, source: &S) -> Refreshed;
    pub fn lines<S: ExplorerSource<Model = M>>(&self, source: &S, width: usize, focused: bool, theme: &Theme) -> GridLines;
    pub fn window(&self) -> ScrollWindow;
    pub fn keep_window(&mut self, window: ScrollWindow, entries_shown: usize);
}

/// What a refresh found.
pub enum Refreshed {
    /// The content was rebuilt; the drill, the view, and the cursor stay.
    Kept,
    /// The thing shown is gone. The host opens the explorer again on
    /// what its own selection names.
    Gone,
}
```

### 2.5 The rules

1. **Cursor.** The cursor is an index into the entries (every zone's,
   in order) and the key of the row there. `open` puts it on the first
   entry, whichever zone it is in (today the first field, so the
   Preview shows the node's own preview). Moves clamp at the ends
   (house rule 2, `Ends::Clamp`). A page moves by the entries the last
   frame showed, at least one (house rule 12).
2. **Reseat after a rebuild.** Every rebuild (a fold, the "every
   property" switch, a refresh, a `Back`) takes the cursor's key first
   and puts the cursor back by the first rule that applies: the entry
   with the same key; else the first entry of the first zone that
   `lands`; else the first entry; else no cursor. Conspectus's field
   zone does not land, so a cursor that stood on a relationship row is
   never dropped into the fields (today's `reseat_cursor`). This is not
   the tree's rule (same identity, else same position): a position
   means nothing across zones whose lengths change, and the tree's rule
   would put a lost link cursor on a field.
3. **Sections.** A section is a `Row::Disclosure` entry and the entries
   whose `under` names its key. `ToggleSection` on a header flips its
   key in `view.open`; on any other row it is refused
   (`Refusal::NotInSection`). `OpenSection` on a closed header opens
   it; `CloseSection` on an open header, or on a row under one, closes
   it and puts the cursor on the header. A refresh drops open keys that
   no longer name a header (today's "no Other rows, so not expanded").
4. **Every property.** `ToggleFull` flips `view.full` and rebuilds. When
   the entries' keys did not change it says so
   (`Refusal::NothingMore`) and leaves the switch flipped, as today.
5. **Drill.** `Activate` on an entry whose grid row reports
   `Intent::Follow` asks `follow` for the far end, then `load`s it. It
   pushes a frame (the current id, tag, view, cursor key, and
   `source.save()`), shows the new content with a default view and the
   cursor on its first entry, and answers `Drill`. A far end that does
   not load answers `Follow` and changes nothing. A link with no far end
   is refused (`Refusal::NoFarEnd`). `drill_to` is the same move by id,
   for the host's own drills (quick select following a path).
6. **Back.** `Back` with frames pops the newest, loads it, restores its
   view, reseats on its cursor key, and answers `Back` with the frame's
   `saved`. A popped frame whose thing does not load is dropped and the
   next older one is tried; when none loads, the explorer stays as it
   is and answers `Refused(Refusal::Gone)`. Today a gone hop takes a
   press of its own (it is dropped with a warning and the next press goes
   on).
7. **Armed back.** `Back` with no frames arms the explorer and answers
   `Armed`; `Back` while armed disarms and answers `Leave`. Any other
   key the explorer handles disarms it. The host disarms it on every
   other message, as `reduce` does today, so a frame drawn between the
   presses leaves the arm alone and anything the operator does clears
   it.
8. **Refresh.** `refresh` reloads the content for the explorer's own
   id, keeps the frames, the view (less vanished section keys), and the
   cursor (reseated), and answers `Kept`; when the id no longer loads
   it answers `Gone`. The host calls `refresh` when its data changed and
   its own selection did not, and `open` when its selection names
   something else (CSP-789 makes the host do this today). Frames are not
   reloaded until `Back` reaches them.
9. **Window.** The explorer keeps the window its zone drew. The host
   passes `window()` to the layout and hands the result back with
   `keep_window` from its viewport report (ADR 0156: windows come back
   by value, not as messages).
10. **Drawing.** `lines` lays every zone out in the source's grid: the
    zone's heading as a line that is no row, then its entries; the
    cursor in the active or inactive look by `focused` (house rules 1
    and 10, already true since CSP-727 and CSP-754). It returns the
    grid's `GridLines`, which the host places in its layout.

### 2.6 The host adapter (a sketch)

```rust
// src/tui/explorer/source.rs (host)
pub(crate) struct Graph;

impl ExplorerModel for Graph {
    type Id = NodeId;
    type Key = ExplorerRowKey;
    type Row = ExplorerRow;
    type Content = NodeView;
    type Saved = Option<RowId>;
}

pub(crate) struct GraphSource<'a> {
    pub snapshot: &'a GraphSnapshot,
    pub home: Option<&'a Path>,
    pub now: Option<i64>,
    pub theme: &'a Theme,
    pub edge_meta: bool,
    pub left: Option<&'a RowId>,
}

impl ExplorerSource for GraphSource<'_> {
    type Model = Graph;
    fn load(&self, id: &NodeId) -> Option<NodeView> {
        build_node_view(ExplorerInputs { snapshot: self.snapshot, target: id, home: self.home, now: self.now })
    }
    fn id(&self, view: &NodeView) -> NodeId { view.focused.clone() }
    fn zones(&self, view: &NodeView, frame: &FrameView<ExplorerRowKey>) -> Vec<Zone<Graph>> {
        // Fields (lands: false), then, when the node has neighbors, the
        // Related zone under its rule (lands: true) with the validated
        // rows, then the Other header and, while it is open, its rows
        // (`under: Some(ExplorerRowKey::OtherHeader)`). The rows come from
        // `explorer::grid`, as they do today.
    }
    fn follow(&self, view: &NodeView, entry: &Entry<Graph>) -> Option<NodeId> {
        view.drill_target(&entry.host)
    }
    fn tag(&self, view: &NodeView) -> String { view.short_label.clone() }
    fn save(&self) -> Option<RowId> { self.left.cloned() }
    fn grid(&self) -> PropertyGrid { explorer::grid::grid(self.theme) }
}
```

Inside `impl App`, the reducer builds the source from fields the
explorer does not live in (`self.handle`, `self.config`, `self.theme`,
`self.edge_meta_visible`, `self.tree_state`) and calls
`self.explorer.as_mut()`; the borrows are disjoint. If an accessor
method gets in the way, the explorer is taken out of the `App` for the
call and put back, as the modal stack is lent out (ADR 0161).

### 2.7 What stays in Conspectus

The node builders and `NodeView`, `CoreField`, `FieldValue`;
`ExplorerRowKey`, `NeighborRowKey`, `ExplorerRow` (as `Key` and `Row`);
`directional_verb`, the tones' roles, kind glyphs and colors; the
`Related` rule's summary words; the Preview (`row_preview`, the edge and
evidence lines, the neighbor's preview); the search hit block; the
left-pane mirror and the restore of the left selection; the edge meta
flag; the right pane's title parts (focus marker, kind glyph and label,
back mark, depth); and every word the status line shows for an outcome.

## 3. The outcome type

### 3.1 The type

```rust
/// What a key asks the host to do. The explorer has already done its
/// own part (moved, folded, drilled, gone back).
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome<M: ExplorerModel> {
    /// Nothing for the host: the cursor moved, or the key did nothing
    /// and needs no word (`Enter` on a placeholder).
    None,
    /// Put `text`, a property's whole value, on the clipboard.
    Copy { label: String, text: String },
    /// Show `text` whole in a reader titled `label`.
    Inspect { label: String, text: String },
    /// A section opened or closed.
    Toggle { open: bool },
    /// The explorer drilled into `to`.
    Drill { to: M::Id },
    /// A link leads to `to`, which the source could not load here.
    Follow { to: M::Id },
    /// The explorer went back to `to`; `saved` is what the host kept
    /// when it drilled away from it.
    Back { to: M::Id, saved: M::Saved },
    /// `Back` with no frames: the next `Back` leaves.
    Armed,
    /// The explorer gives up the keys (the task called it `LeaveRight`;
    /// the framework does not know which side its pane is on).
    Leave,
    /// The key did not do what it is for; the host says why.
    Refused(Refusal),
}

/// Why a key did nothing, for the host to word (house rule 7: no
/// silent rejection).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A fold key away from a section.
    NotInSection,
    /// The "every property" switch changed no row.
    NothingMore,
    /// `Enter` on a link with no far end.
    NoFarEnd,
    /// The inspect key on a row with nothing cut.
    NothingToInspect,
    /// `Back` found no frame whose thing still loads.
    Gone,
}
```

The task listed eight outcomes (`Copy`, `Inspect`, `Drill`, `Back`,
`LeaveRight`, `Toggle`, `None`, `Follow`). The design keeps all eight
(with `LeaveRight` named `Leave`) and adds two: `Armed`, because the
host shows the arm's hint in its own words, and `Refused`, because every
refusal today is a status line the host writes and the framework cannot
hold the host's copy.

### 3.2 Keys, intents, outcomes, and what the host does

| Key (`ExplorerKey`) | Cursor row | Grid intent | Outcome | Conspectus does |
| --- | --- | --- | --- | --- |
| `Activate` | property with a value | `Copy` | `Copy { label, text }` | `Msg::Copy { text, done: "copied: <label>" }`, so `Effect::Copy` writes OSC 52 and toasts |
| `Activate` | placeholder | none | `None` | nothing (ADR 0187) |
| `Activate` | section header | `Toggle` | `Toggle { open }` | nothing |
| `Activate` | link whose far end loads | `Follow` | `Drill { to }` | mirror the left pane onto `to`; reset the Preview scroll |
| `Activate` | link whose far end does not load | `Follow` | `Follow { to }` | warning: "drill target … not in current snapshot" |
| `Activate` | inert link (unresolved stub) | none | `Refused(NoFarEnd)` | warning (today's words; CSP-788 corrects them) |
| `Inspect` | cut property | `Inspect` | `Inspect { label, text }` | open the value modal (`Pager`) |
| `Inspect` | any other row | none | `Refused(NothingToInspect)` | warning "row has no truncated value to open" |
| `ToggleSection` | section header | — | `Toggle { open }` | nothing |
| `OpenSection` | closed header; open header | — | `Toggle { open: true }`; `None` | nothing |
| `CloseSection` | open header, or a row under it; closed header | — | `Toggle { open: false }` (the cursor lands on the header); `None` | nothing |
| a fold key | any other row | — | `Refused(NotInSection)` | warning "nothing to expand here …" |
| `ToggleFull` | any | — | `None` or `Refused(NothingMore)` | warning "this node kind has no extra fields (N total)" |
| `Down`, `Up`, `PageDown`, `PageUp`, `First`, `Last` | any | — | `None` | clear the status line |
| `Back` | frames left | — | `Back { to, saved }` | select `saved` in the left tree; reset the Preview scroll |
| `Back` | no frames, not armed | — | `Armed` | info "no drill history — press Backspace again to return to the left pane" |
| `Back` | no frames, armed | — | `Leave` | `focus = Left` |
| `Back` | frames, none loads | — | `Refused(Gone)` | warning "cannot restore breadcrumb hop …" |

### 3.3 Against the overlay conventions

The explorer is a pane, not an overlay: it is never on the modal stack,
an open modal still owns every key (ADR 0161), and the right pane's
table is resolved only when no modal is open. Its outcome lines up with
`overlay::Outcome` where the shapes agree:

| `explorer::Outcome` | Closest `overlay::Outcome` | Why it is not the same |
| --- | --- | --- |
| `None` | `Consumed` | the same: the control used the key |
| `Copy`, `Inspect`, `Drill`, `Follow`, `Back`, `Toggle` | `Stay(M)` | the control stays and the host acts; the explorer reports facts instead of carrying the host's `M`, so the framework needs no message type for a pane |
| `Leave` | `Close` | a pane gives up focus; nothing closes |
| `Armed`, `Refused` | `Consumed` plus a note | overlays set a note in their footer; a pane's note is the host's status line |

House rule 4 (`Esc` steps back one level) carries over: `Esc` is an
alias of `Back`. `q` and `Ctrl-C` are not the explorer's: they quit the
program, so the right pane's table neither binds them nor starts with
`Ctrl-C` (the overlay tables' check does not apply to it; section 6.1,
risk 10). Cancelling is silent (house rule 5): `Leave` posts nothing.

### 3.4 Against the property grid's intents

The grid reports what a row asks for; the explorer is the host the grid
page names ("A grid's own key table joins it when the explorer that owns
its cursor moves into the framework"):

| `property_grid::Intent` | From | The explorer |
| --- | --- | --- |
| `Copy { label, text }` | `activate` on a property with a value | passes it on as `Outcome::Copy` |
| `Inspect { label, text }` | `inspect` on a cut property | passes it on as `Outcome::Inspect` |
| `Toggle` | `activate` on a disclosure row | does the fold itself, answers `Toggle { open }` |
| `Follow` | `activate` on a link that is not inert | asks `follow` and `load`, answers `Drill` or `Follow` |
| none | a placeholder, an inert link, `inspect` on an uncut row | `None`, `Refused(NoFarEnd)`, or `Refused(NothingToInspect)` |

## 4. Breadcrumb and the stacked layout

### 4.1 One fit rule for pieces of a line, and `Breadcrumb` on it

Three surfaces fit pieces into a room and mark what gave way: the
breadcrumb (a ladder of four forms, `explorer.rs`), the tab strip
(`tab_strip_fit`, CSP-716: inactive names drop from the end, ` …` ends
the strip), and the status bar (pieces drop by priority, ` …` ends
it). They share one rule in `tuikit::text`:

```rust
/// One piece of a line, measured in cells, and when it gives way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Piece {
    pub width: usize,
    /// `None` keeps the piece whatever the room; otherwise pieces give
    /// way lowest rank first.
    pub rank: Option<u32>,
}

/// Where the mark for pieces that gave way goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkAt {
    /// In the gap they left, as an item between separators (a breadcrumb).
    Gap,
    /// Once, at the end (a tab strip, a status bar).
    End,
}

/// What fits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Elided {
    /// The pieces kept, by index, in order.
    pub kept: Vec<usize>,
    /// Indices into `kept` before which a mark stands (`MarkAt::Gap`).
    pub marks: Vec<usize>,
    /// Whether a mark ends the line (`MarkAt::End`).
    pub end_mark: bool,
    /// When even the kept pieces do not fit: the cells the last kept
    /// piece may take, cut with `text::truncate_end`. Marks are dropped
    /// then.
    pub cut: Option<usize>,
}

pub fn elide(pieces: &[Piece], separator: usize, mark: usize, room: usize, at: MarkAt) -> Elided;
```

The rule: start from every piece; while the line (kept pieces, a
separator between each two items, a mark per dropped run at `Gap` or one
at `End`) is wider than the room and a piece with a rank is kept, drop
the lowest-ranked one. When only unranked pieces are left and they still
do not fit, drop the marks and cut the last kept piece to the room.
Widths are cells (house rule 14).

`Breadcrumb` (`tuikit::breadcrumb`) draws hops on it:

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Hop {
    /// A one-cell glyph before the tag, in the host's style.
    pub glyph: Option<Span<'static>>,
    pub tag: String,
    /// Shown after the tag, joined with `·`, only when another hop has
    /// the same tag.
    pub suffix: String,
}

pub struct Breadcrumb<'a> { /* hops, style */ }

impl<'a> Breadcrumb<'a> {
    pub fn new(hops: &'a [Hop]) -> Self;
    /// The style of the tags, separators, and mark (the host's `inert`).
    pub fn style(self, style: Style) -> Self;
    /// The chain in at most `room` cells, or `None` with no hops.
    pub fn line(&self, room: usize) -> Option<Line<'static>>;
}
```

Ranks: the last hop is unranked (always shown); the first hop ranks
above every middle hop; middle hops rank oldest lowest. The separator is
` › ` (a new `marks::CRUMB`, `›`, since the framework draws no character
outside `glyphs`), the mark `…` (`marks::ELLIPSIS`) at `Gap`. So a chain
narrows as `a › b › c › d › e`, `a › … › c › d › e`, `a › … › d › e`,
`a › … › e`, `… › e`, then `e` cut with `…`. Today's ladder jumps from
the whole chain to `a › … › e`, ends with the last hop alone at its full
width, and floors the room at 8 cells so the border clips the title
below that (CSP-790 lists both changes).

How the three share code: `elide` is the one implementation. `Breadcrumb`
ranks hops as above with `MarkAt::Gap`. `TabBar` (CSP-758, which now
depends on CSP-790) gives the active tab no rank and inactive tabs ranks
that drop from the end (today's `tab_strip_fit`), with `MarkAt::End`.
`PriorityLine` passes the status bar's priorities as ranks, with
`MarkAt::End`. Each keeps its own separator and mark spacing (` · ` and
` …` for the strip and the bar).

Conspectus's adapter builds a `Hop` per frame: the kind glyph in its
color from the frame's `NodeId` (a PR keeps `pr_open`, as today), the
tag from the frame's `tag` with its `kind:` prefix dropped, and the last
four characters of the id as the suffix. The title keeps its marker,
kind glyph and label, ` ◄ `, and ` · depth N`, and gives the chain the
room left (no floor).

### 4.2 `StackedMasterDetail` and `SectionRule`

`tuikit::master_detail::StackedMasterDetail` is the right pane's layout
today, as parameters:

```rust
pub struct StackedMasterDetail {
    min_list: u16,   // 3
    min_detail: u16, // 6
}

/// What the layout drew.
pub struct Stacked {
    /// The list's window, for the host to keep.
    pub window: ScrollWindow,
    pub list: Rect,
    pub divider: Rect,
    /// Where the host draws the detail.
    pub detail: Rect,
}

impl StackedMasterDetail {
    pub const fn new() -> Self;
    pub const fn min_list(self, rows: u16) -> Self;
    pub const fn min_detail(self, rows: u16) -> Self;
    /// The width the host lays the list's lines out at: the area's less
    /// the column a scrollbar may take, so a rule never wraps when the
    /// bar appears.
    pub fn layout_width(area: Rect) -> usize;
    /// The three areas for `rows` wrapped list rows, without drawing.
    pub fn areas(&self, area: Rect, rows: usize) -> (Rect, Rect, Rect);
    /// Wrap the list's lines, size the zones, draw the list in a window
    /// (from `window`) that follows the cursor line's rows, with the
    /// shared gutter, and the divider line under it.
    pub fn render(
        &self,
        area: Rect,
        buf: &mut Buffer,
        list: GridLines,
        divider: Line<'static>,
        window: ScrollWindow,
    ) -> Stacked;
}
```

The arithmetic is today's, generalized (`divider` is one row):

- `floor = min(min_detail, height − (min_list + 1))`;
- `max_list = max(height − (floor + 1), min_list)`;
- the lines wrap with `WrappedRows::fit(lines, width, max_list)` (full
  width when they fit, one column less when they overflow);
- `list = clamp(rows, min_list, max_list)`, then the divider, then the
  detail gets the rest.

With `min_list` 3 and `min_detail` 6 that is `MIN_PREVIEW_HEIGHT`, the
`inner.height − 4` fallback, and the 3-row floor of `draw_right_panel`,
so CSP-791's frames are byte-identical. The host keeps what goes in:
the search hit block spliced above the explorer's lines (it shifts the
cursor line), the divider's words, and the Preview it draws into
`detail`.

`tuikit::frame::SectionRule` is `chip_divider_line` without its domain
name: a rule in `muted`, the label as a chip in `accent` under `badge`,
an optional summary in `inert` before it (`summary_first`) or after it,
and a two-cell closing rule; when the chip and summary leave no room the
leading rule drops out. The explorer's zone headings (`Related` with its
counts) and the layout's divider (`Preview`) are section rules. The
inventory's other remarks on the rule (the `Left` anchor's doc
contradicting the code, the stranded doc paragraph) are fixed by the
move.

## 5. Migration plan

Each step builds, passes the full suite, and leaves `main` releasable on
its own. Steps 0 to 2 are separate stories that CSP-756 now depends on;
steps 3 to 6 are CSP-756's own commits.

| Step | Story | Lands | Visible change | Why `main` stays releasable |
| --- | --- | --- | --- | --- |
| 0 | CSP-789 | Host: the pane remembers the left selection it reflects; a refresh or projection change that leaves it alone reloads the focused node in place; a test pins `Back` to a gone node | a drill survives the 30-second refresh | a host-only fix with reducer tests |
| 1 | CSP-790 | Framework `text::elide`, `marks::CRUMB`, `breadcrumb::Breadcrumb`, its page; host title draws through it; the 8-cell floor goes | the chain keeps more hops and cuts the last one instead of being clipped | the old function is replaced in one place, frames checked at four widths |
| 2 | CSP-791 | Framework `frame::SectionRule`, `master_detail::StackedMasterDetail`, their pages; `draw_right_panel` and both rules use them | none: byte-identical | pure refactor behind identical frames |
| 3 | CSP-756 | Framework `tuikit::explorer` core (section 2) with its tests over a toy source, `ExplorerKey`, `EXPLORER_KEYS`, `KeyAction` and `HelpAction` impls, its page. No host change | none | additive; nothing uses it yet |
| 4 | CSP-756 | Host adopts it: `explorer::Graph` and `GraphSource`; `App.explorer: Option<Explorer<Graph>>`; `ExplorerState`, `explorer_window`, `explorer_back_armed` go; eight `Msg::Explorer*` become `Msg::Explorer(ExplorerKey)`; `Action::ExplorerEnter` and `runtime::explorer_enter` go (a copy is `Effect::Copy`; the snapshot's copy skip posts the toast); keys still arrive through `remap_for_focus`, mapped to the new message | none in the live TUI (frames byte-identical); in `--snapshot` the value modal's and Messages' skipped copies now show their toast like `Enter` does | the reducer tests and frames prove it; revertible alone |
| 5 | CSP-756 | `RIGHT_PANE_KEYS` resolved first on right focus, with the focus guard; `remap_for_focus`'s right arm and the global `e` row go; help section and right-focus status hint generated from the table; the key meanings of section 7.1 | the key changes listed in section 7.1; reworded help rows and hint | each change is listed and checked with `--snapshot`; tests pin each key |
| 6 | CSP-756 | Docs: `explorer.md` page, ADR 0188 amended with what was learned, `docs/design.md`, `docs/operations.md`, `CHANGELOG.md` for the key changes; the README and module map | none | docs |

**The first step, precisely (CSP-789, host only).** Add a field to
`App`, the left selection the explorer reflects, set by
`recompute_explorer` when it opens the pane, by `mirror_left_pane_to`
after it selects the drilled row, and by `explorer_back` after it
restores a frame's selection. In `recompute_explorer`, when
`tree_state.selected()` equals it, rebuild the view for
`explorer.view.focused` (not the selection's node) and keep the
breadcrumb, `other_expanded`, `full_detail_expanded`, and the cursor
through `reseat_cursor`; when the focused node no longer builds, open on
the selection's node. Otherwise open anew as today. Add a test that
pins `Backspace` to a gone node (it warns, and the next press goes on).
The failing test is the probe of section 10 (drill into the mux from
`app_for_explorer`, `SetData` with the same snapshot, assert the
focused node and the depth).

As landed (CSP-789): the field is `App::explorer_reflects`, the
in-place reload is `App::reload_drill` over `ExplorerState::reload`,
and it applies only to a drilled pane (an undrilled one reloads the
selection's node as before). `set_selection`, the path of every jump
to a row (a pick, quick select's `here`, a regroup, a follow, an
undo's restore), forgets the field first, so a jump onto the row
already selected opens the pane on it again: the adapter calls `open`
there, not `refresh`. Six reducer tests in `app_tests.rs` (the
refresh, a sort and filter change, a pick of the selected row, a move,
a gone drilled node, and `Backspace` to a gone frame) join the 26 that
section 6.2 ports.

**The first framework step (CSP-790).** Create
`crates/tuikit/src/text/elide.rs` with `Piece`, `MarkAt`, `Elided`, and
`elide`, re-exported from `tuikit::text`; add `CRUMB` (`›`) and
`SPACED_CRUMB` (` › `) to `glyphs::marks` and `marks::ALL`; create
`crates/tuikit/src/breadcrumb.rs` with `Hop` and `Breadcrumb` and
`crates/tuikit/docs/controls/breadcrumb.md`; in the host, replace
`render_breadcrumb_chain`, `build_breadcrumb_line`, `spans_for_hops`,
and `styled_glyph` with a `breadcrumb_hops(&[BreadcrumbHop], &Theme) ->
Vec<Hop>` adapter and call `Breadcrumb::new(&hops).style(inert).line(room)`
from `right_panel_title` without `available.max(8)`.

As landed (CSP-790): `elide` is as specified, with `Piece::ranked` and
`Piece::unranked` constructors, ties going later piece first (the status
bar's order), and `Elided::items` and `Elided::width` so a caller
interleaves separators and marks the same way; when the marks go, the
last unranked piece is cut only if it still does not fit. `Hop` is
built (`Hop::new(tag).glyph(span).suffix(id)`), not a struct literal,
and `Breadcrumb::new(&hops, &theme)` takes the theme and defaults to
its `inert` (`.style` still overrides). The repeat test compares the
tags drawn, so two kinds with one name now carry suffixes (the host
compared `kind:tag`). The title was handed the pane's whole width, not
the cells its corners leave, which is how a full title lost its pad and
sometimes the depth's digit; it now gets the inner width, measures its
fixed parts from the spans it draws, and drops its trailing pad before
any hop. The five generic tests are framework tests with the new rule's
expectations; the kind color test and a new tag and suffix test are
adapter tests in `explorer_tests.rs`; a title test in `ui_tests.rs`
checks the fit at every width and that the corner clips nothing.

**The second framework step (CSP-791), as landed.** `crates/tuikit/src/frame/rule.rs`
holds `SectionRule` (`new(label)`, `.summary(text)`, `.summary_first()`,
`.line(width, &theme)`, `CLOSING`), re-exported from `tuikit::frame`;
`crates/tuikit/src/master_detail.rs` holds `StackedMasterDetail` and
`Stacked` as section 4.2 sketches them, with `MIN_LIST` and `MIN_DETAIL`
as the defaults and `Default`. `render` takes `Line<'_>` for the
divider. The split is arithmetic rather than a ratatui `Layout`: it
matches the solver everywhere, including an area under the list's
minimum and the divider (the list keeps its first row, then the
divider its row, then the list the rest). An empty summary shows
nothing (the host drew two spaces; it never passed one). The host keeps
`MIN_PREVIEW_HEIGHT` and one `LAYOUT` constant, lays its lines out at
`layout_width`, and drops `chip_divider_line`, `ChipAnchor`, and
`preview_divider_line`'s unused `App`. 261 frames at 160x40, 100x30, and
60x20, focused and unfocused, are byte-identical; the framework has 16
layout tests and 8 rule tests. The rule still runs past a width below
its chip, summary, and closing rule (the explorer wraps it onto a
second row, as at 40 columns); giving way there is a seam on the frame
page and CSP-794.

**The first step of CSP-756 itself (step 3).** Create
`crates/tuikit/src/explorer.rs` (types and docs) with
`explorer/state.rs` (`Explorer`), `explorer/keys.rs` (`ExplorerKey`,
`EXPLORER_KEYS`), and `explorer/outcome.rs`; a toy source in
`explorer_tests.rs` (a map from ids to contents with a field zone that
does not land, a link zone that does, and one section) and one test per
rule of section 2.5 and per row of the table in section 3.2;
`crates/tuikit/docs/controls/explorer.md` from the template; the module
map and README rows.

## 6. Risks and the tests that guard behavior

### 6.1 Risks

| # | Risk | Mitigation | Guarded by |
| --- | --- | --- | --- |
| 1 | The refresh defect would be baked into the framework's refresh rule | fix it first (CSP-789); the framework rule 8 is the fixed behavior | CSP-789's reducer tests; framework refresh tests |
| 2 | Borrowing: the explorer lives in `App` and the source borrows other `App` fields | build `GraphSource` from fields, not accessor methods; otherwise take the explorer out for the call (ADR 0161's lending) | the compiler; app tests drive every key through `App::update` |
| 3 | Keys the right table does not bind fall through to the global table and move the left tree (`Alt-j`), and the explorer's `Enter` swallows `Shift-Enter`/`Alt-Enter` (reveal) | a focus guard drops the global table's left-tree actions on right focus; the explorer's `Enter` is `Exact { NONE, Enter }` | runtime tests per key, including `Shift-Enter`, `Alt-Enter`, `Alt-j` on right focus |
| 4 | The copy moves from the runtime to `Effect::Copy`; the snapshot driver skips copies and would lose the `copied:` toast | the snapshot's copy skip posts the copy's `done` toast without writing | `key_prelude_o_and_enter_reach_a_truncated_field_past_the_core_rows`; the app tests assert `Effect::Copy` |
| 5 | Arm clearing changes if it moves wholly into the explorer (a refresh between the presses would no longer disarm) | the host keeps "every other message disarms" as one `disarm()` call | `explorer_back_armed_state_clears_on_intervening_message`, `explorer_back_arm_survives_the_frames_between_presses` |
| 6 | The reseat rule drifts toward the tree's (position fallback) and drops a lost link cursor into the fields | rule 2 with the source's `lands`; `open` still starts on the first entry | framework reseat tests; `explorer_state_initializes_with_cursor_on_the_first_node_field`, the full-detail tests |
| 7 | Host drills without a key (quick select's path, its "open the Other zone to find a winner") lose their seam | `drill_to`, `back`, and `seek` | `consistency_tests/search_quick.rs` (3 tests) |
| 8 | Rows are built per key and per frame with styled spans | it is today's cost (`explorer_cursor_row` builds a grid row per key); cache by content and view only if a profile asks | none needed now |
| 9 | Readers of the cursor row (the Preview's subject, `action_subject`, `preview_focus`) break | `cursor_entry(source).host` is the `ExplorerRow` they read today; `cursor()` and `id()` feed `preview_focus` | the right pane's ui tests, `consistency_tests/actions.rs` (4), `focus.rs` (2) |
| 10 | The key-table registry assumes overlay tables (`Ctrl-C` first) | register the right pane's table as a pane table: `mislabeled`, `unresolved`, and `loose_characters` only | the extended registry test in `keybindings/overlay_tests.rs` |
| 11 | The breadcrumb's visible change surprises | listed in CSP-790; `--snapshot` at 160, 100, 60, 40 | the ported breadcrumb tests with their new expectations |
| 12 | A domain word leaks into the framework (refusal wording, "node", "drill target") | the framework's types and docs say thing, link, section, property; every word stays in the host | the independence guard |
| 13 | Generated help and hint rows change wording | listed in CSP-756; the stale `e` text goes | the help parity and drift tests |
| 14 | Status hint longer than today's drops earlier on a narrow bar | the hint keeps only move, `Enter`, fold, and back (`Footer::only`) | status bar tests at narrow width |

### 6.2 Porting the tests (none dropped)

| Where | Tests | What happens to them |
| --- | --- | --- |
| `explorer_tests.rs` (1,706 lines, 36 tests) | 29 builder tests (verbs, sort, per-kind fields, cwd owner, short labels, agreeing producers, lineage) | stay, unchanged: they test `NodeView`, which stays |
| | `flat_rows_splits_validated_and_other_zones` | becomes an adapter test of `GraphSource::zones` (same assertions over keys and zones); the fold rule gets a framework test |
| | `neighbor_row_key_survives_a_change_of_representative_link` | stays (the key is the host's choice); a framework test checks the reseat by key |
| | six `render_breadcrumb_chain_*` tests | CSP-790: five become `tuikit::breadcrumb` tests (join, none when empty, elide the middle, extremely narrow, colliding tags), the elision and narrow ones with the new rule's expectations, stated in the summary; the kind-color one stays as an adapter test |
| `app_tests.rs` (26 explorer tests) | home and end, first cursor, drill, two mirror tests, two restores, the left pane's hint, three arm tests, unwind then arm, edge meta, two full-detail tests, the Other toggle, the clamp, six copy and inspect tests, the reset on a new selection, two copy-target tests | stay as host integration tests through `App::update(Msg::Explorer(..))`, renamed mechanically; the copy tests assert `Effect::Copy` instead of an injected clipboard; the left pane's hint test moves to `Msg::NavBack`; each rule they pin also gets a framework test over the toy source, without an `App` or a `GraphSnapshot` |
| `ui_tests.rs` (18 right-pane tests) | title, related rows, edge meta, layout and scrollbar, Preview of a related row, drilling, the two `Backspace` tests | unchanged; the layout ones guard CSP-791's byte identity; `backspace_on_the_right_pane_leaves_the_back_stack_alone` keeps its meaning, and a new test pins finding 2 (left `Backspace` leaves the right pane's drill alone) |
| `runtime_tests.rs` (6 tests) | `translate_e_…`, five `remap_for_focus_…` | become resolution tests of `RIGHT_PANE_KEYS` on right focus and the focus guard; new cases for the changed keys |
| `snapshot_tests.rs` (2 tests) | drill and back; `o` and `Enter` on a cut field | unchanged expectations (risk 4) |
| consistency suite (10 tests) | right pane subject (4), focus (2), unfocused cursor (1), quick select's paths (3) | unchanged |
| new framework tests | `text::elide` (about 12), `breadcrumb` (about 8), `master_detail` (about 8), `SectionRule` (about 4), `explorer` (about 30) | each rule of sections 2.5, 3.2, and 4 |

`explorer_tests.rs` stays a sibling test file of about 1,550 lines.

## 7. The keys that mean different things by focus

### 7.1 Every right-pane key

| Key | Left pane (global table) | Right pane today | Right pane after CSP-756 (`ExplorerKey`) | Changes |
| --- | --- | --- | --- | --- |
| `Enter` | default action: attach, view, scrollback, or fold | copy a field, drill a link, fold `Other`, a hint on an unresolved stub | `Activate`: the same | none (the copy is an effect) |
| `F` | clear every filter | Expanded Node Detail | `ToggleFull`: the same | none |
| `Backspace` | reveal history back; with none, pops the right pane's drill (finding 2), else a hint | pop a frame; with none, arm, then move focus left | `Back`: the same | the left pane no longer pops the right pane's drill; it shows its hint |
| `Esc` | nothing | nothing | alias of `Back` | gains `Back` (house rule 4: `Esc` steps back one level) |
| `e` | nothing | fold `Other` on its header, a hint elsewhere | `ToggleSection`: the same | none |
| `h`, `←` | collapse the tree row | nothing | `CloseSection`: close the section the cursor heads or sits in, landing on its header | gains a meaning (the tree's fold gesture) |
| `l`, `→` | expand the tree row | nothing | `OpenSection`: open the section the cursor heads | gains a meaning |
| `PgDn`, `PgUp` | page the tree by its window | one row | `PageDown`, `PageUp`: by the rows the zone shows | pages (house rule 12) |
| `j`, `k`, `↓`, `↑` | move | move | `Down`, `Up` | `Alt-j` and `Alt-k` stop moving the explorer (ADR 0160's character rule) |
| `g`, `G`, `Home`, `End` | first, last row | first, last row | `First`, `Last` | none |
| `o` | opens the right pane's cursor row's value | the same | `Inspect` | none; the left pane keeps its reach (finding 4) |
| `E`, `i`, `O`, `Shift-Enter`, `Alt-Enter`, `Tab`, `J`, `K` | global | global | stay global; the explorer's `Enter` is exact so the reveal chords reach the global table | none |

### 7.2 The table that declares them

```rust
// tuikit::explorer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExplorerKey {
    Down, Up, PageDown, PageUp, First, Last,
    Activate, Inspect,
    ToggleSection, OpenSection, CloseSection,
    ToggleFull,
    Back,
}

/// An explorer: `↑/↓ move · Enter open · e fold · o inspect ·
/// F show all · Backspace back`.
pub const EXPLORER_KEYS: &[KeyRow<ExplorerKey>] = &[
    shown(key(KeyCode::Up), ExplorerKey::Up, marks::ARROW_UP, "move"),
    alias(ch('k'), ExplorerKey::Up),
    shown(key(KeyCode::Down), ExplorerKey::Down, marks::ARROW_DOWN, "move"),
    alias(ch('j'), ExplorerKey::Down),
    alias(key(KeyCode::PageUp), ExplorerKey::PageUp),
    alias(key(KeyCode::PageDown), ExplorerKey::PageDown),
    alias(key(KeyCode::Home), ExplorerKey::First),
    alias(ch('g'), ExplorerKey::First),
    alias(key(KeyCode::End), ExplorerKey::Last),
    alias(ch('G'), ExplorerKey::Last),
    shown(exact(KeyCode::Enter), ExplorerKey::Activate, "Enter", "open"),
    shown(ch('e'), ExplorerKey::ToggleSection, "e", "fold"),
    alias(ch('l'), ExplorerKey::OpenSection),
    alias(key(KeyCode::Right), ExplorerKey::OpenSection),
    alias(ch('h'), ExplorerKey::CloseSection),
    alias(key(KeyCode::Left), ExplorerKey::CloseSection),
    shown(ch('o'), ExplorerKey::Inspect, "o", "inspect"),
    shown(ch('F'), ExplorerKey::ToggleFull, "F", "show all"),
    shown(key(KeyCode::Backspace), ExplorerKey::Back, "Backspace", "back"),
    alias(key(KeyCode::Esc), ExplorerKey::Back),
];
```

`exact(code)` is a new `const` builder for `KeyMatcher::Exact` with no
modifiers. `KeyAction` classifies `Down`..`Last` as `Move` (`PageDown`,
`PageUp` as `Page`), `Activate` as `Apply`, the fold keys and
`ToggleFull` as `Change`, `Back` as `Leave`, `Inspect` as `Other`.
`HelpAction` gives each a sentence in the framework's words; the host's
help page describes them in its own (as a pager content's `describe`
does).

In Conspectus, `keybindings::RIGHT_PANE_KEYS` is the declaration of the
right pane's meanings:

```rust
pub const RIGHT_PANE_KEYS: &[KeyRow<ExplorerKey>] = &[include(EXPLORER_KEYS)];
```

and `keymap::INSPECT_KEY` is checked against the `Inspect` row by a
test, so the grid's `(truncated · o)` hint and the binding cannot drift.
The left pane's meanings of the same keys stay in `KEYBINDINGS`. When
CSP-760 builds the scoped keymap, `RIGHT_PANE_KEYS` becomes its
"right pane" scope and the two meanings of `F`, `Enter`, and `Backspace`
sit in one scoped table, which is what that story asks for.

The Enter verb is the weak point of a static table: `Enter` copies,
follows, or folds by row. The table's verb is `open`; the status bar's
right-focus hint is the explorer's own footer, which takes the table's
segments and gives `Enter` the cursor row's verb (`copy`, `follow`,
`fold`) and draws `o inspect` unavailable on a row with nothing cut
(ADR 0160 decision 7). CSP-756 may settle on the static verb instead if
the contextual one proves fussy; either way it is generated.

### 7.3 How the host resolves it

With no modal open and the right pane focused, the runtime resolves
`RIGHT_PANE_KEYS` first and dispatches `Msg::Explorer(key)`; a key it
does not bind goes to the global table. A focus guard then drops the
global actions that belong to the left tree (`NavDown`, `NavUp`,
`PageDown`, `PageUp`, `Home`, `End`, `ExpandRow`, `CollapseRow`,
`DefaultAction`, `ClearFilters`, and the left pane's back), so a chord
the right table refuses (`Alt-j`) does nothing instead of moving the
tree behind the operator. This replaces `remap_for_focus`'s right arm;
its left arm goes with the global `e` row. The fixture loop and the
snapshot driver take the same path, so the three loops keep one
dispatch.

## 8. GO or NO-GO

**GO.** Reasons:

1. The control flow is generic and the seam is narrow. Measured, about
   750 lines of host code (the explorer's state, the cursor, fold,
   detail, drill, back, and arm transitions, the breadcrumb fit, the
   layout, the rule, the remap, the runtime's `Enter` branch) are
   replaced by framework code; everything that knows the graph crosses
   the seam as an id, a key, a content value, or a saved selection.
2. It removes problems the inventory counted, not just lines: the
   `Action::ExplorerEnter` branch in three loops, three hand-written
   descriptions of the right pane's keys (one stale), an arm whose
   clearing rule lives on `App`, and 26 state-machine tests that each
   build a `GraphSnapshot` because the state cannot exist without one.
3. It finishes the property grid: the grid reports intents and defers
   its key table to "the explorer that owns its cursor".
4. The risk is staged. Steps 0 to 2 are small, useful whatever happens
   later, and two of them are byte-identical; step 4 is a refactor
   proved by unchanged frames and reverted alone; the key changes are
   last and listed one by one.
5. It needs no dependency and keeps the decisions already made: ADR 0151
   kept the explorer in scope as the last extraction, and ADRs 0156,
   0159, 0160, 0161, and 0187 already give it its window, cursor look,
   key model, the rule that an open modal owns every key, and its rows;
   ADR 0182 gives the precedent of a cursor kept by identity across the
   host's rebuilds.

Against it, weighed: the explorer has one consumer, so its reuse is a
bet on the framework's purpose (the tree has one consumer too); its
payoff is lower than the other controls' (ADR 0151); and the drill is
tied to the left pane through the mirror, which the outcomes keep in
the host rather than pretending it is generic.

**Stop rule.** If step 4 cannot keep the 26 app tests' assertions and
the right pane's frames unchanged without more than the lending pattern
(for example, if the Preview or quick select need the explorer's
internals), revert step 3 (it is additive, so the revert is clean) and
take the NO-GO path for the rest; the framework keeps no control its
host does not use.

**The NO-GO path, if taken.** The explorer's state stays in Conspectus
as `ExplorerState`. What still extracts: `text::elide` and `Breadcrumb`
(CSP-790) and `StackedMasterDetail` and `SectionRule` (CSP-791), which
do not depend on the core. The right pane's keys are still declared in
one host table, `RIGHT_PANE_KEYS` over a host enum, resolved as in
section 7.3, and the outcome type is kept host-internal so the
`ExplorerEnter` branch still goes. CSP-756 then closes with the decision
recorded in its final summary and an amendment to ADR 0188, and its
acceptance criteria 1 and 2 are met by the no-go reason and the
host-side outcome.

## 9. Destination table (checked against the code)

### 9.1 `App` fields

See section 1.6; every field the state machine reads or writes is
listed there with its destination.

### 9.2 `Msg` variants

| `Msg` today | Used for | Destination |
| --- | --- | --- |
| `ExplorerNavDown` | `j`, `↓` (and `PgDn`) on the right | `Msg::Explorer(ExplorerKey::Down)` (`PageDown` its own) |
| `ExplorerNavUp` | `k`, `↑` (and `PgUp`) | `Explorer(Up)` (`PageUp` its own) |
| `ExplorerHome` | `g`, `Home` | `Explorer(First)` |
| `ExplorerEnd` | `G`, `End` | `Explorer(Last)` |
| `ExplorerActivate` | `Enter` after the runtime's copy check | `Explorer(Activate)` |
| `ExplorerToggleGroup` | `e` | `Explorer(ToggleSection)` |
| `ExplorerToggleFullDetail` | `F` on the right | `Explorer(ToggleFull)` |
| `ExplorerBack` | `Backspace` on either pane | right: `Explorer(Back)`; left: a new host `Msg::NavBack` (reveal history, else its hint) |
| `ToggleEdgeMeta` | `E` | stays (host, global) |
| `CycleFocus` | `Tab` | stays; `Outcome::Leave` sets the same focus |
| `PageDown`, `PageUp` | page keys on the left | stay for the left; the right pane's are `Explorer(PageDown/PageUp)` |
| `ExpandRow`, `CollapseRow` | `h`/`l`/arrows on the left (dropped on the right) | stay for the left; the right pane's are `Explorer(OpenSection/CloseSection)` |
| `Copy { text, done }` | value modal and Messages copies | also carries `Outcome::Copy` |
| `SetStatus` | hints | stays; the outcome's words |
| `SetData`, `SetTree`, `SelectRow`, nav moves (callers of `recompute_explorer`) | rebuilds | stay; they call `Explorer::refresh` when the left selection is unchanged and `Explorer::open` otherwise (CSP-789) |
| `RevealSelected` | `O`, `Shift-Enter`, `Alt-Enter` | stays; reads `cursor_entry` and `id` through `action_subject` |
| `OpenEntity` | quick select's pick | stays; drills with `drill_to`, undoes with `back`, lands with `seek` |

### 9.3 `Action` variants

| `Action` today | Destination |
| --- | --- |
| `ExplorerEnter` | deleted: `Enter` on the right is `Msg::Explorer(Activate)`; the copy is `Effect::Copy` |
| `OpenValueModal` | `Msg::Explorer(Inspect)` on either pane (today's reach); `Outcome::Inspect` opens the value modal |
| `DefaultAction` | the left pane's `Enter` only (the right table takes `Enter` first) |
| `ClearFilters` | the left pane's `F` only |
| `CopySessionId` | stays (`i`, on the focused pane's subject) |
| `Msg(..)` | as in section 9.2 |
| `ModalKey` | unchanged: an open modal owns every key |

### 9.4 Functions

| Today | Destination |
| --- | --- |
| `explorer_move_cursor`, `explorer_jump_cursor_to` | `Explorer::handle` |
| `explorer_toggle_group`, `explorer_toggle_full_detail`, `explorer_activate` | `Explorer::handle` plus the host's words for `Refused` |
| `explorer_drill_into` | `Explorer::drill_to`; the host mirrors on `Drill` |
| `explorer_back` | `Explorer::back`; the host restores `saved` and shifts focus on `Leave` |
| `mirror_left_pane_to`, `toggle_edge_meta`, `recompute_explorer` | stay (host) |
| `recompute_explorer_for` | `GraphSource::load` plus `Explorer::refresh`/`open` |
| `ExplorerState::{new, rows, selected_row, reseat_cursor}` | `Explorer::{open, entries, cursor_entry}` and rule 2 |
| `explorer_cursor_row`, `explorer_copy_target`, `open_value_modal_for_cursor` | `Outcome::Copy` and `Outcome::Inspect` and their host mapping |
| `NodeView::flat_rows`, `has_other_rows`, `fields` | `GraphSource::zones` |
| `ExplorerRow::key`, `is_other_header` | `Entry::key`; `Row::Disclosure` |
| `NodeView::drill_target` | `GraphSource::follow` |
| `render_breadcrumb_chain` and its helpers | `Breadcrumb` plus the host's hop adapter |
| `render_explorer_lines`, `related_rule` | `Explorer::lines` plus the zone's `SectionRule` |
| `draw_right_panel`'s layout | `StackedMasterDetail` |
| `chip_divider_line`, `ChipAnchor` | `frame::SectionRule` |
| `remap_for_focus` | right arm: `RIGHT_PANE_KEYS` and the focus guard; left arm: gone with the `e` row |
| `runtime::explorer_enter`, `explorer_enter_action` | deleted |
| help section "Detail-pane graph explorer (right focus)" | generated from `RIGHT_PANE_KEYS` |
| `contextual_status_segments`' right-focus string | the explorer's footer |

## 10. Probes run for this design (not committed)

- A snapshot binary at `a5af2807` (`--features snapshot`) and a capture
  script over the showcase fixture
  (`/tmp/tuifw-baseline/scripts755/probe.sh`, frames in
  `/tmp/tuifw-baseline/frames-755-probe/`): a session's right pane, a
  Related row, one and two drills, a three-hop drill at 40 columns
  (`◄ … › ◇ project · depth 2`), the armed `Backspace` and its second
  press, an intervening key, `Esc`, `h`, and `PgDn` on the right pane,
  and `Tab` then `Backspace` after a drill (finding 2). Snapshot mode's
  `r` runs real discovery, so it could not show the refresh defect.
- A reducer test appended to `src/tui/app_tests.rs`, run alone with
  tmux isolated, then reverted with `git checkout`: drill from
  `app_for_explorer` into the mux, `SetData` with the same snapshot; it
  failed with the pane back on the session at depth 0 (finding 1).
- A line counter (`/tmp/tuifw-baseline/scripts755/measure.py`) for the
  tables of section 1.
