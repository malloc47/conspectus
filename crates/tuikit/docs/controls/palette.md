# `Palette<R>`

| | |
| --- | --- |
| Module | `tuikit::palette` |
| Layer | 1 (control) |
| Type parameters | `R`, the host's row (`PaletteRow`; the built-in `Item<T>` carries a value `T`); `M`, the host's message, which the host's `on_pick` builds from the picked row; `A`, the action enum of the palette's key table (`PaletteKey` unless the host has keys of its own, through `PaletteAction`); the palette implements `Overlay<M, Cx>` for every context `Cx` and reads none |
| Cargo feature | none |
| Outcomes | whatever `on_pick` answers for `Enter` on a row (a host commits its message with `Commit(M)`, or stays with `Stay(M)`); `Close` for `Enter` with nothing listed and for `Esc`; `Dismiss` for `Ctrl-C` (and `q` in the options pane); `Consumed` for a move, an edit, a scope switch, an option change, a host action handed to it, and every other key |

## Purpose

`Palette<R>` picks one thing by typing part of its name: a query line over
a ranked list of results, with a cursor, a footer of its keys, and a pick.
A filter picker over known candidates, a quick jump to anything by name, a
search whose results a slow provider sends, and a command palette are all
instances. Its parts:

- **The query**, a `LineEdit` drawn by `TextField` after its prompt (`/`)
  with its caret while the results have the keys.
- **The title**: the palette's name, then its scope strips (a segmented
  choice such as `[ Here ] Everywhere`), then a count of the results.
- **Ranked results**, ranked by the host, drawn through the shared
  selection look, wrapping at the ends, in a scroll window with the shared
  scrollbar.
- **A status row** under the query, when the host sets one.
- **The footer**, generated from the palette's key table.
- **An options pane** of named on/off settings, opened over the results.
- **A debounce** for a slow provider.

It replaces what grows palette by palette: one query line with no caret
and one with, a picker whose cursor stays at its position as the list
narrows beside a search whose cursor goes to the top, a list with no
footer and no paging, a frame sized by its own rule, rows hard-cut at the
frame, and a scroll window that re-follows from the top every frame.

The host keeps the candidates or the provider, the ranking, the title's
words, the notes and the status, what each scope and option means, and
what a pick commits: `on_pick` gets the row and answers with the host's
message.

## Usage example

A host that jumps to a room filters its rooms by every word typed and
commits the room picked.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tuikit::overlay::{Outcome, Overlay};
use tuikit::palette::{Item, Palette, every_word};
use tuikit::testkit::buffer::rows;
use tuikit::theme::Theme;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Open(u32),
}

let rooms = vec![
    Item::new("kitchen", 1).detail("ground floor"),
    Item::new("attic", 2).detail("top floor"),
    Item::new("cellar", 3).detail("below ground"),
];
let mut palette = Palette::new("Go to", rooms, |room: &Item<u32>| {
    Outcome::Commit(Msg::Open(room.value))
})
.ranked_by(every_word);
let press = |code| KeyEvent::new(code, KeyModifiers::NONE);

// Typing narrows the list to the rows with every word.
for c in "ground".chars() {
    palette.handle(press(KeyCode::Char(c)), &());
}
let listed: Vec<&str> = palette.listed().map(|room| room.label.as_str()).collect();
assert_eq!(listed, ["kitchen", "cellar"]);

// Down moves to the cellar; Enter hands it to the host.
palette.handle(press(KeyCode::Down), &());
assert_eq!(palette.handle(press(KeyCode::Enter), &()), Outcome::Commit(Msg::Open(3)));

let theme = Theme::default();
let mut buf = Buffer::empty(Rect::new(0, 0, 70, 16));
palette.draw(buf.area, &mut buf, &theme);
let drawn = rows(&buf).join("\n");
assert!(drawn.contains("Go to  (2/3)"), "{drawn}");
assert!(drawn.contains("► cellar  · below ground"), "{drawn}");
assert!(drawn.contains("↑/↓ move · Enter pick · Esc close"), "{drawn}");
```

The same example is the doctest on the `palette` module, so it compiles.
The doctest on `Palette` routes keys through a `ModalStack` and settles
the pick. The unit tests show a host table with keys of its own, scope
strips, the options pane, and the debounce.

### Rows

A row is the host's type implementing `PaletteRow`:

- `cells(&RowContext) -> Vec<Span<'static>>` lays the row out. The
  context says how many cells the row has (after the cursor gutter, and
  after the scrollbar's column when the results overflow), whether the
  cursor is on it (`Cursor`, so the row can tell focused from not), the
  query as typed (for a row that marks what matched), and the theme.
  `RowContext` is built by the palette and can carry more later; outside
  the crate it is built with `RowContext::new`.
- `haystack() -> Cow<str>` is what the built-in filter matches. It is
  empty unless the row says otherwise, so a row whose palette ranks it
  some other way need not say.

The palette draws each row as `selection::row(fit_spans(cells, width),
cursor, theme)`: the pointer in the 2-cell gutter, the cells cut to the
row with `…` in cells, and the theme's selection pair on the cursor's
row. So every palette's rows look and move alike, whatever their cells.

`Item<T>` is the built-in row: a label, an optional detail, and the
host's value `T`. It draws the label, two spaces, the separator mark,
and the detail, receding in the theme's `muted`; the detail is cut before
the label is. Its haystack is the label and the detail.

### Ranking

A palette over known candidates takes a ranker with `ranked_by`: a
function of the query and every row that answers the indices of the rows
to list, best first. The palette reranks on every edit and whenever its
rows change. `every_word` is the built-in ranker: the rows whose haystack
holds every word of the query, ignoring case, in the host's order; an
empty query lists every row.

A palette fed by a provider takes no ranker: its rows are the results, in
the order given. The host hands each batch over with `set_rows` (after a
search it ran, or with another set of candidates when the scope changes).

**The cursor.** An edit (typing, deleting, `set_query`), a scope switch,
and an option change put the cursor on the first result, the best one. New
rows from `set_rows` keep the cursor where it was while that is still a
result, and put it on the first otherwise, so a growing list of streamed
results does not move it. `at(index)` starts the cursor on a given row.

### Title, status, and notes

The title is the palette's name, then each scope strip, then the count,
two cells apart: `Go to  [ Here ] Everywhere  (2/3)`. The count is
`Count::OfTotal` (`(listed/total)`, the default, for a palette that
filters its own candidates), `Count::Listed` (`(listed)`, for results a
host ranked), or `Count::None`.

The first strip (`scopes`) is the palette's scope: the `Scope` key
switches it to its next choice, wrapping. A palette may have more strips
(a second choice of how results are grouped); the host sets those with
`set_scope`. `scope(strip)` reads either.

`set_status` puts a row under the query (how far a scan has got),
receding and cut with `…`. An empty list says `empty_note` while nothing
is typed (`(nothing to pick)` unless set) and `no_match_note` when the
query matches nothing (`(no matches)` unless set), receding; a host can
change either while the palette is open (`(searching…)` while a provider
works).

### The options pane

`options(Options::new([("Hidden files", false), ...]))` gives the
palette named settings, each on or off, that change what the results are.
The `Options` key (`Ctrl-O` in a host table) opens the pane over the
results with its cursor on the first option; the query loses its caret and
the pane takes the keys of `PALETTE_OPTION_KEYS`: the arrows (and `k`/`j`,
`Ctrl-P`/`Ctrl-N`) move, wrapping; `Space` or `Enter` turns the option
under the cursor on or off; `Esc` or `Ctrl-O` goes back to the results;
`q` and `Ctrl-C` dismiss the palette. Each option is a row through
`selection::row` with its box (`[x]` or `[ ]`); the status row stays and
the footer becomes the pane's. A change puts the results' cursor on the
first and owes a debounced palette's host a search. The host reads and
restores settings with `options_pane()` and `options_mut()`.

### The debounce

A palette whose results a slow provider sends takes `debounced(delay)`.
Every edit, scope switch, and option change marks its `Debounce`; the host
polls `take_due(now, force)` from its loop, and when it answers `true`
(once per rest, or at once with `force`) starts its search with what the
palette shows (`query()`, `scope(0)`, the options) and hands the results
back with `set_rows`. `mark_edited(at)` owes a search the palette did not
see an edit for (a palette opened with a query). The framework keeps only
the timer; what runs the search, and on which thread, is the host's. A
palette without a debounce is never due.

### Size and scrolling

A palette is the palette frame (`PALETTE_SIZE`, which is
`ModalSize::PALETTE`: 85% of the screen's width, 60 to 200 columns, and
80% of its height, 12 to 50 rows, keeping two columns and a row clear
while the screen allows), and fills it whatever the results hold, so the
frame keeps its size as they narrow. `size` sizes a palette some other
way; with a size that follows its body, the palette asks for its query,
status row, every result, and footer.

Inside the frame, from the top: the query; the status row, when there is
one; the results (or the note, or the options pane); the footer. The
footer keeps the bottom rows, at most four, and leaves the query, the
status row, and one row of results above it. The results scroll in a
`ScrollWindow` the palette keeps between draws, moving no further than the
cursor needs (so the cursor rides the bottom row as it moves down, and
moving back up stays in the window). Results that overflow give the last
column to the shared scrollbar (`ScrollGutter`) and are laid out one cell
narrower. `window()` is the window of the last draw; `PgUp` and `PgDn`
move the cursor by its rows.

### Keys of the host's own

A palette with a scope, an options pane, or a verb of its own for `Enter`
takes its own table with `keys`. A table over `PaletteKey` builds on the
standard one:

```rust
use ratatui::crossterm::event::KeyCode;
use tuikit::keys::{self, KeyRow, PALETTE_KEYS, PaletteKey, ctrl, include, key, shown};

const FIND_KEYS: &[KeyRow<PaletteKey>] = &[
    include(PALETTE_KEYS),
    shown(key(KeyCode::Tab), PaletteKey::Scope, "Tab", "look here/everywhere"),
    shown(ctrl('o'), PaletteKey::Options, "Ctrl-O", "set options"),
];
assert_eq!(
    keys::footer(FIND_KEYS),
    "↑/↓ move · Enter pick · Esc close · Tab look here/everywhere · Ctrl-O set options"
);
```

A host with actions the palette does not know (a verb that keeps the
query, one that recalls an earlier query) writes its table over its own
enum and implements `PaletteAction` for it: `palette_key()` says which of
its actions are the palette's (`Some(PaletteKey::Up)`) and which are its
own (`None`). The host's overlay resolves its own actions first with
`host_action(key)` and hands the rest to the palette, which swallows a
host action it is handed rather than typing it. `action(key)` is the
action a key resolves to at the results' level, for a host that must act
before the palette does (build what a wider scope searches before the
switch). `footer()` is the footer as drawn.

## Key table

`PALETTE_KEYS`, over `PaletteKey` (a standard table; see
[keys.md](keys.md)).

| Key | Action | Notes |
| --- | --- | --- |
| `Ctrl-C` | Abort | dismisses the palette (`Dismiss`), saying nothing; an alias, not in the footer |
| `↑`, `Ctrl-P` | Up | the previous result, wrapping from the first to the last |
| `↓`, `Ctrl-N` | Down | the next result, wrapping from the last to the first |
| `PgUp` | PageUp | up by the rows shown, stopping at the first result; an alias |
| `PgDn` | PageDown | down by the rows shown, stopping at the last result; an alias |
| `Enter` | Pick | hands the row under the cursor to `on_pick` and reports its answer; closes when nothing is listed |
| `Esc` | Cancel | closes the palette (`Close`), saying nothing |
| (host table) | Scope | switches the first scope strip; no standard row binds it (`Tab` by convention) |
| (host table) | Options | opens the options pane; no standard row binds it (`Ctrl-O` by convention) |
| any other key | (none) | edits the query, so `q`, `j`, `k`, and `Space` type |

`PALETTE_OPTION_KEYS`, over `PaletteOptionKey`, while the options pane is
open:

| Key | Action | Notes |
| --- | --- | --- |
| `Ctrl-C`, `q` | Abort | dismisses the palette; nothing is typed in the pane |
| `↑`, `k`, `Ctrl-P` | Up | the previous option, wrapping |
| `↓`, `j`, `Ctrl-N` | Down | the next option, wrapping |
| `Space`, `Enter` | Toggle | turns the option under the cursor on or off |
| `Esc`, `Ctrl-O` | Back | back to the results |
| any other key | (none) | swallowed |

The footers read `↑/↓ move · Enter pick · Esc close` and `↑/↓ move ·
Space toggle · Esc back`.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | the frame's border and title; the query's prompt |
| `surface` | the background the frame paints |
| `field_cursor` | the query's caret, while the results have the keys (`TextField`) |
| `selection_active` | the cursor's row, gutter to the end of its cells (`selection::row`); an option's row in the pane |
| `muted` | the status row, the empty list's note, the footer, and an `Item`'s detail |
| `badge` (with `accent`) | the chosen choice of a scope strip (`SegmentedTitle`) |

A host's row takes whatever tokens its cells use. The scrollbar is the
shared `ScrollGutter` look.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | each result goes through `selection::row`: the pointer in the 2-cell gutter and the theme's selection pair over the gutter and cells |
| 2 | List ends | a palette is a short choice list: the arrows wrap (`Ends::Wrap`); the page keys stop |
| 4 | Close keys | `Esc` closes the palette's first level and steps back from the options pane; `Ctrl-C` dismisses from either; the query types `q`, and the pane, where nothing is typed, dismisses on it |
| 5 | Close feedback | closing returns `Close` or `Dismiss` and carries no message |
| 8 | Footers and hints | the footer is generated from the palette's table (or the pane's) in the one grammar and keeps the bottom rows |
| 10 | Unfocused cursor | an open palette has the keys, so its cursor row takes `selection_active`; the query shows no caret while the pane has the keys |
| 12 | Paging | `PgUp`/`PgDn` move by the rows shown and stop at the ends; `Home`/`End` move the query's caret, and `g`/`G` type |
| 13 | Theme | the frame, the prompt, the caret, the highlight, the status, the notes, and the footer take theme tokens; no raw color or reversed cell |
| 14 | Width | rows are cut with `…` in cells, the detail before the label; the status and notes are cut with `…`; the footer breaks between hints |
| 15 | Text editing | the query is the one `LineEdit` with the shared editing keys, drawn by `TextField` with its window in cells |

Not applicable: 3 (a pick is the list itself; a host that has more to say
asks a dialog after it), 6 and 7 (one field and no validation; the host's
`on_pick` decides what a pick means), 9 (the focus cue is the pane
frame's), 11 (the palette reports no feedback; a host's status row is its
own words), 16 (no path field), 17 (labels and notes are the host's
copy), and 18 (pagers).

## Seams left open

- **A command palette.** An instance over rows built from a key table's
  shown rows: `keys::rows(table)` gives each bound row's action and hint,
  so `Item::new(verb, action).detail(label)` lists the verb with its key
  and `every_word` filters them; `on_pick` commits the row's action, and a
  row's availability (`KeyRow::Bind { available, .. }`) can recede or
  hide it. The program-wide keymap a later step adds would be its source.
  No adapter is built.
- **A row preview.** A pane beside or under the results showing the
  cursor's row (a file's start, a command's help). The body lays the
  results out in one place, so a split of the list's area would go there,
  and `PaletteRow` would gain a method with a default that says what to
  show. Not built.
- **A plan of columns.** Rows with several cells (a kind, a label, a
  context, a word at the right edge) lay themselves out from the width
  they are told. A plan shared by every row (ordered cells with minimum
  and maximum widths and a drop order) would travel in `RowContext`,
  which only the palette builds.
- **A progress status.** The status row is the host's text; a formatter
  for a scan's progress (`scanned/total`, skipped, done) would sit beside
  the debounce.
- **Match highlights.** A row gets the query and paints what matched
  itself (`text::paint` with the theme's `match_style`); a ranker that
  answers ranges with its indices would save the second search.
- **Several picks.** One pick per palette; marking several rows would
  share the checklist's marks and a `Space` that is no longer typed.
- **Mouse hit testing.** The frame's rect is the palette size's, and each
  result is one row of the window, which a click would be tested against.
