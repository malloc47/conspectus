# `Explorer`

| | |
| --- | --- |
| Module | `tuikit::explorer` |
| Layer | 1 (component) |
| Type parameters | `M: ExplorerModel`: the host's model, naming the `Id` of a thing, the `Key` of a row, the host's own `Row`, the `Content` shown for one thing, and what the host keeps (`Saved`) with a frame; a source `S: ExplorerSource<Model = M>` the host builds for each call |
| Cargo feature | none |
| Outcomes | `Outcome<M>`: `None`, `Copy { label, text }`, `Inspect { label, text }`, `Toggle { open }`, `Drill { to }`, `Follow { to }`, `Back { to, saved }`, `Armed`, `Leave`, or `Refused(Refusal)`; `refresh` answers `Refreshed::{Kept, Gone}` |

## Purpose

A pane that shows one thing lists its properties above the links that
lead from it to other things, and lets its reader move a cursor over the
rows, fold a section of them, show every property instead of a summary,
follow a link to the thing at its far end, and come back. Written into
the program that owns the data, that control is a cursor kept by index
and by a row key, a reseat rule after each rebuild, a fold flag, a
switch, a stack of drill frames, a two-press `Back` whose arm lives on
the program's state, a keymap that rewrites the other pane's keys, and a
branch in each event loop for the one key that copies: none of it
testable without the program and its data.

`Explorer<M>` keeps that state and its rules. It reads the host's data
only through an `ExplorerSource`, built for each call from what the host
holds: `load` a thing, its `id`, its `zones` of rows for a view, where a
link row leads (`follow`), its breadcrumb `tag`, what to `save` with a
frame, and the property grid its rows draw in. It answers every key with
an `Outcome` and does no effect itself.

The host keeps what its things are and how each is built, every row's
key, content, verb, and look (as property grid rows), what it saves with
a frame, every word of a hint or refusal, and every effect: the
clipboard a copy goes to, the reader a value opens in, what a drill
changes outside the pane, where focus goes when the explorer leaves. It
also decides when its data changed: `refresh` while the thing it asked
for is still the one shown, `open` when it asks for another.

## Usage example

A catalog whose items refer to other items. The source builds a zone of
properties and, under a rule, a zone of links that a lost cursor lands
on; the host resolves its keys through `EXPLORER_KEYS` and acts on each
outcome.

```rust
use std::collections::BTreeMap;

use ratatui::crossterm::event::KeyCode;
use tuikit::explorer::{
    EXPLORER_KEYS, Entry, Explorer, ExplorerKey, ExplorerModel, ExplorerSource, FrameView,
    Outcome, Zone,
};
use tuikit::frame::SectionRule;
use tuikit::keys::resolve;
use tuikit::property_grid::{LinkRow, PropertyRow};
use tuikit::testkit::keys::press;
use tuikit::theme::Theme;

/// A catalog of items, each naming the items it refers to.
struct Catalog;

impl ExplorerModel for Catalog {
    type Id = String;
    type Key = String;
    type Row = ();
    type Content = (String, Vec<String>);
    type Saved = ();
}

struct Items(BTreeMap<String, Vec<String>>);

impl ExplorerSource for Items {
    type Model = Catalog;

    fn load(&self, id: &String) -> Option<(String, Vec<String>)> {
        self.0.get(id).map(|refs| (id.clone(), refs.clone()))
    }

    fn id(&self, item: &(String, Vec<String>)) -> String {
        item.0.clone()
    }

    fn zones(&self, item: &(String, Vec<String>), _: &FrameView<String>) -> Vec<Zone<Catalog>> {
        let name = Entry::new("name".to_string(), PropertyRow::new("name", item.0.clone()), ());
        let links = item
            .1
            .iter()
            .map(|to| Entry::new(format!("to {to}"), LinkRow::new("refers to", to.clone()), ()))
            .collect();
        vec![
            Zone::new(vec![name]),
            Zone::new(links).heading(SectionRule::new("Links")).lands(),
        ]
    }

    fn follow(&self, _: &(String, Vec<String>), entry: &Entry<Catalog>) -> Option<String> {
        entry.key.strip_prefix("to ").map(str::to_string)
    }

    fn tag(&self, item: &(String, Vec<String>)) -> String {
        item.0.clone()
    }

    fn save(&self) {}
}

let items = Items(BTreeMap::from([
    ("a".to_string(), vec!["b".to_string()]),
    ("b".to_string(), Vec::new()),
]));
let mut explorer = Explorer::open(&"a".to_string(), &items).expect("a loads");
assert_eq!(explorer.cursor(), Some(0), "the cursor starts on the first row");

// The host resolves a key through the table and does what the outcome
// asks: here, nothing after a move, and its own follow-up of a drill.
let key = |code| resolve(EXPLORER_KEYS, press(code)).expect("bound");
assert_eq!(explorer.handle(key(KeyCode::Down), &items), Outcome::None);
let drill = explorer.handle(key(KeyCode::Enter), &items);
assert_eq!(drill, Outcome::Drill { to: "b".to_string() });
assert_eq!(explorer.frames().len(), 1);

// Back returns to the link it left from; with no frame left it arms,
// and a second press leaves.
let back = explorer.handle(key(KeyCode::Backspace), &items);
assert_eq!(back, Outcome::Back { to: "a".to_string(), saved: () });
assert_eq!(explorer.cursor(), Some(1));
assert_eq!(explorer.handle(ExplorerKey::Back, &items), Outcome::Armed);
assert_eq!(explorer.handle(ExplorerKey::Back, &items), Outcome::Leave);

// A property, the rule over the links, and a link.
let drawn = explorer.lines(&items, 40, true, &Theme::default());
assert_eq!(drawn.lines.len(), 3);
assert_eq!(drawn.cursor_line, Some(2));
```

The same example is the module's documentation test, so it compiles.

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `ExplorerModel` | names `Id`, `Key`, `Row`, `Content`, `Saved` | usually a unit struct; the explorer's types implement `Clone`, `Debug`, and `PartialEq` over these, so the model derives nothing |
| `ExplorerSource` | `load`, `id`, `zones`, `follow`, `tag`, `save`, `grid` | built for each call from the host's data; `grid` defaults to `PropertyGrid::new()` |
| `IdOf<S>`, `KeyOf<S>`, `ContentOf<S>`, `SavedOf<S>` | a source's model types | for writing a source's signatures |
| `Zone::new(entries)`, `.heading(rule)`, `.lands()` | a run of rows, under an optional `SectionRule`, that a lost cursor may land on | fields `heading`, `lands`, `entries` are public |
| `Entry::new(key, row, host)`, `.under(header)` | one row: its key, its property grid row, the host's row, and the section header it sits under | fields `key`, `row`, `under`, `host` |
| `FrameView { open, full }`, `is_open(key)` | how a frame shows its content: the open sections by header key, and the every-property switch | `Default` is every section closed and the summary |
| `Frame { id, tag, view, cursor, saved }` | a frame of drill history | `frames()` lists them oldest first |
| `Explorer::open(id, source)` | the explorer on `id`, cursor on the first row | `None` when `id` does not load |
| `content`, `id`, `tag`, `view`, `cursor`, `cursor_key`, `frames`, `is_armed` | what it shows and how | `cursor()` is the index among every zone's entries |
| `zones(source)`, `entries(source)`, `cursor_entry(source)` | the rows as the source lays them out now | the cursor's entry hands back the host's row (`host`) |
| `handle(key, source)` | does what an `ExplorerKey` asks, answers an `Outcome` | every key but `Back` disarms |
| `drill_to(id, source)`, `back(source)` | the drill and the back a host makes without a key (following a path, undoing it) | answer as `Enter` on a link and `Back` do |
| `seek(source, find)` | puts the cursor on the first entry `find(content, entry)` accepts, opening a closed section that holds one | says whether it found one |
| `disarm()` | forgets an armed `Back` | the host calls it for everything else it handles |
| `refresh(source)` | loads what is shown again, keeping the frames, the view, and the cursor | answers `Kept` or `Gone` |
| `lines(source, width, focused, theme)` | every zone in the source's grid: headings as lines that are no row, the cursor in the active or inactive look | returns the grid's `GridLines` |
| `window()`, `keep_window(window)` | the window the zone drew, kept between frames | a page key moves by its rows |
| `footer(table, source)` | the keys a status line names: move, `Enter` with its verb on the cursor's row, fold, back | `Enter copy`, `Enter follow`, `Enter fold`; the table's verb, unavailable, where `Enter` does nothing |
| `hint(table)` | the same keys with the table's verbs | `↑/↓ move · Enter open · e fold · Backspace back`; for a status line with no explorer to read |
| `ExplorerKey`, `EXPLORER_KEYS` | the keys and their one table | implement `KeyAction` and `HelpAction` |

### The rules

1. **Cursor.** An index into the entries of every zone in order and the
   key of the row there. `open` and a drill put it on the first entry,
   whichever zone it is in. Moves stop at the ends; a page moves by the
   rows the zone showed in the last frame, at least one.
2. **Reseat.** After every rebuild (a fold, the every-property switch, a
   refresh, a `Back`), the cursor goes to the entry with its key, else to
   the first entry of the first zone that `lands`, else to the first
   entry, else to none. A position means nothing across zones whose
   lengths change, so unlike a tree the explorer keeps no position: a
   zone of properties that does not land keeps a cursor that stood on a
   link among the links.
3. **Sections.** A section is a header row (`Row::Disclosure`) and the
   rows whose `under` names its key. `ToggleSection` flips it on its
   header and is refused elsewhere; `OpenSection` opens a closed header;
   `CloseSection` closes an open one from its header or from a row
   inside it, and puts the cursor on the header. A fold key with nothing
   to do (opening an open section) answers `None`; one on a row in no
   section is refused (`NotInSection`). A refresh and a `Back` drop the
   open keys that no longer name a header.
4. **Every property.** `ToggleFull` flips `view.full` and rebuilds; when
   no row's key changed it says so (`NothingMore`) and leaves the switch
   flipped.
5. **Drill.** `Activate` on a row whose grid row asks to follow it asks
   the source where it leads and loads that thing: it pushes a frame
   (what was shown, its tag, its view, the cursor's key, and
   `source.save()`), shows the new thing with the default view and the
   cursor on its first entry, and answers `Drill`. A far end that does
   not load answers `Follow` and changes nothing; a link with no far end
   (an inert link, or one the source cannot name) is refused (`NoFarEnd`).
6. **Back.** With frames, the newest is popped, loaded, its view
   restored, and the cursor reseated on its key; the answer is `Back`
   with what the host saved. A frame whose thing no longer loads is
   dropped and the next older one tried in the same press; when none
   loads, what is shown stays and the answer is `Refused(Gone)`.
7. **Armed back.** `Back` with no frames arms and answers `Armed`; `Back`
   while armed answers `Leave`. Every other key disarms, and the host
   disarms on everything else it handles, so a frame drawn between the
   presses leaves the arm alone and anything the reader does clears it.
8. **Refresh.** `refresh` loads what is shown again, keeps the frames,
   the view (less the sections that are gone), and the cursor
   (reseated), and answers `Kept`; when the thing no longer loads it
   answers `Gone` and changes nothing. Frames are loaded only when `Back`
   reaches them.
9. **Window.** The explorer keeps the window its zone drew; the host
   hands it to its layout and keeps what the layout returns with
   `keep_window`. A drill and a `Back` keep it, so the layout moves it as
   little as it must to show the new cursor.
10. **Drawing.** `lines` lays every zone out in the source's grid: the
    heading as a line that is no row, then the rows, the cursor in the
    active look when `focused` and the inactive look when not.

### Outcomes and what a host does with them

| Key | Cursor's row | Outcome | A host |
| --- | --- | --- | --- |
| `Activate` | property with a value | `Copy { label, text }` | puts `text` on its clipboard and says what it copied |
| `Activate` | property with none | `None` | nothing |
| `Activate` | section header | `Toggle { open }` | nothing |
| `Activate` | link whose far end loads | `Drill { to }` | follows up outside the pane (moves another pane's selection, say) |
| `Activate` | link whose far end does not load | `Follow { to }` | says it cannot be shown |
| `Activate` | link with no far end | `Refused(NoFarEnd)` | says so |
| `Inspect` | cut property | `Inspect { label, text }` | opens `text` in a reader (a `Pager`) |
| `Inspect` | any other row | `Refused(NothingToInspect)` | says so |
| fold keys | header, or a row under an open one | `Toggle { open }` or `None` | nothing |
| fold keys | a row in no section | `Refused(NotInSection)` | says so |
| `ToggleFull` | any | `None` or `Refused(NothingMore)` | says so for the refusal |
| moves | any | `None` | clears its note |
| `Back` | frames left | `Back { to, saved }` | restores what it saved |
| `Back` | none, not armed | `Armed` | says the next press leaves |
| `Back` | none, armed | `Leave` | moves focus away, silently |
| `Back` | frames, none loads | `Refused(Gone)` | says so |

The explorer is a pane, not an overlay: it is never on a modal stack, and
a host resolves its table only while no modal is open. Its outcome lines
up with `overlay::Outcome` where the shapes agree (`None` is
`Consumed`; `Leave` is `Close` for a pane, which gives up focus with
nothing to close), but it reports facts instead of carrying a host
message.

## Key table

`EXPLORER_KEYS` (`ExplorerKey`), which a host resolves before its own
keymap while the explorer has focus. The footer is
`↑/↓ move · Enter open · e fold · o inspect · F show all · Backspace back`;
a status line uses `Explorer::footer`, which keeps move, `Enter`, fold,
and back and gives `Enter` its verb on the cursor's row (`hint` is the
same with the table's verbs). A help page lists every row; a host that
names its rows its own way generates it with
`help::sections_described(table, describe)`.

A host whose other keys give some of these keys other meanings
(`Enter` for its own default action, `h`/`l` for a tree's folds) takes
this table in as the explorer pane's scope of its
[scoped keymap](keys.md#the-scoped-keymap) (`KeymapBuilder::table`), and
puts the other pane's keys in that pane's scope, so both meanings sit in
one table and a chord the explorer's table leaves out (`Alt-j`) does
nothing instead of moving what the reader is not looking at.

| Key | Action | Notes |
| --- | --- | --- |
| `↑`, `k` | `Up` | shared with the standard list tables (`k` an alias) |
| `↓`, `j` | `Down` | as above |
| `PgUp`, `PgDn` | `PageUp`, `PageDown` | by the rows the zone showed (house rule 12) |
| `Home`, `g` / `End`, `G` | `First` / `Last` | aliases |
| `Enter` | `Activate` | bound with no modifier (`keys::exact`), so `Shift-Enter` and `Alt-Enter` reach the host's keymap |
| `e` | `ToggleSection` | on a header |
| `l`, `→` | `OpenSection` | the tree's unfold gesture |
| `h`, `←` | `CloseSection` | the tree's fold gesture, from the header or a row inside |
| `o` | `Inspect` | a host that names its inspect key elsewhere (a grid's cut hint) checks it against this row |
| `F` | `ToggleFull` | |
| `Backspace`, `Esc` | `Back` | `Esc` steps back one level (house rule 4) |

`q` and `Ctrl-C` are not the explorer's: a pane gives way to its
program's quit keys, so the table neither binds them nor starts with
`Ctrl-C` as an overlay's does; it takes the checks a pane's table takes
(`mislabeled`, `unresolved`, `loose_characters`). Characters bind as
typed, so `Alt-j` and `Ctrl-j` are not `j`. The table binds no `Ctrl-N`
and `Ctrl-P`: like a tree, the explorer is a pane that moves with the
arrows and `j`/`k`, where the overlay lists move with those chords too.

## Theme tokens

The explorer reads no token itself. Its rows draw through the property
grid (`selection_active` and `selection_inactive` for the cursor row,
`muted`, `warning`, `link`, `inert`, `disclosure`; see
[property_grid.md](property_grid.md)), and its zone headings through
`SectionRule` (`muted`, `accent`, `badge`, `inert`; see
[frame.md](frame.md)). `Explorer::footer` returns a `Footer`, whose
unavailable segment takes `unavailable_style` (`inert`, `muted`) when the
host draws it with `Footer::lines`.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | the cursor's row goes through the property grid's shared cursor look |
| 2 | List ends | the cursor stops at the first and the last row (`Ends::Clamp`): an explorer is scrollable content |
| 4 | Close keys | `Esc` steps back one level, as `Backspace` does; `q` and `Ctrl-C` stay the program's |
| 5 | Close feedback | `Leave` asks for no word |
| 7 | Validation errors | no key is refused silently: every refusal is a `Refused(Refusal)` the host words |
| 8 | Footers and hints | the help and the status hint come from `EXPLORER_KEYS` (`HelpAction`, `Explorer::footer`) |
| 10 | Unfocused cursor | `lines(.., focused: false, ..)` draws the cursor in the inactive pair |
| 12 | Paging | `PgUp`/`PgDn` move by the rows shown; `Home`/`End`/`g`/`G` go to the ends |
| 13 | Theme | every look is the grid's or the rule's, from the theme |
| 14 | Width | the rows and headings measure cells (the grid's and the rule's) |

Not applicable: 3 (the explorer confirms nothing; the armed `Back` is a
second press, not a confirmation of a change), 6 (no form), 9 (the focus
cue is the pane frame's), 11 (the host chooses each note's severity), 15
and 16 (no text entry), 17 (the host writes every label), and 18 (a value
opens in the host's `Pager`, which follows it).

## Seams left open

- **Several sections.** The rules and the tests take any number of
  sections, but the one host has a single `Other` section; a host with
  nested sections would need a header's `under` to name its parent.
- **Mouse.** `lines` returns the grid's `GridLines`, one line per row or
  heading before any wrap; a host maps a click through its wrapped rows
  to a line, and a later step can map a line to an entry for it.
- **Caching.** The rows are asked for on every key and every frame, which
  is cheap for a few dozen rows. A host with hundreds would cache them by
  content and view; nothing in the explorer assumes it does not.
- **A page by entries.** A page moves by the rows the zone showed. An
  entry that wraps onto two rows makes a page move one entry more than
  showed; counting entries instead needs the layout to report which lines
  it showed.
