# `Pager<C>`

| | |
| --- | --- |
| Module | `tuikit::pager` |
| Layer | 1 (control) |
| Type parameters | `C`, the host's [`PagerContent`](#the-content): its rows, the units a find matches, its header, chips, keys, and the message (`C::Message`) its requests and the copy hook carry. The pager implements `Overlay<C::Message, Cx>` for every context `Cx`: it reads only its content |
| Cargo feature | none |
| Outcomes | `Consumed` for a key it used or swallowed; `Close` for `Esc` at the pager's first level (no help page, prompt, or find open) and for a content key that closes it; `Dismiss` for `q` and `Ctrl-C`; `Stay(M)` for a content request (`PagerEffect::Request`) and for the copy hook's message |

## Purpose

`Pager` is the one reader: any text a person scrolls through, finds in,
and leaves. Programs grow several (a full-screen document reader, a
snapshot of a terminal's scrollback, the help page, a long value, a log's
detail, a preview), and each grows its own scroll offset, page size, end
key, find prompt, wrap-around notes, help page, footer, and close keys,
which then disagree: one pages by a fixed eight rows, one forgets to clamp,
one's `q` closes and another's steps back, one lands a find on the match
and another only when it is off screen.

The pager owns all of that once. The host keeps what its rows are: how
they are laid out for a width (wrapped, numbered, rendered from Markdown),
which units a find matches (a line, an entry, a paragraph) and how they are
painted, the header's words, the content's own keys and what they do (a
request the host carries out, a toggle that lays the rows out again), and
what a copy copies.

## Usage example

A document whose lines are the units a find matches. The pager is drawn,
finds a line, steps back, and leaves.

```rust
use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use ratatui::text::Line;
use tuikit::keys::KeyRow;
use tuikit::overlay::Outcome;
use tuikit::pager::{Find, NoKeys, Pager, PagerContent, PagerEffect};
use tuikit::testkit::buffer::rows;
use tuikit::testkit::keys::{press, typed};
use tuikit::theme::Theme;

struct Document(Vec<String>);

impl PagerContent for Document {
    type Message = ();
    type Action = NoKeys;
    const KEYS: &'static [KeyRow<NoKeys>] = &[];

    fn layout(&mut self, _width: u16, _theme: &Theme) -> usize {
        self.0.len()
    }

    fn rows(&self, range: Range<usize>, _theme: &Theme) -> Vec<Line<'static>> {
        self.0[range].iter().map(|line| Line::from(line.clone())).collect()
    }

    fn header(&self, _width: usize, _theme: &Theme) -> Line<'static> {
        Line::from(" notes")
    }

    fn find(&self, query: &str) -> Vec<usize> {
        let lines = self.0.iter().enumerate();
        lines.filter(|(_, line)| line.contains(query)).map(|(at, _)| at).collect()
    }

    fn paint(&self, rows: &mut [Line<'static>], first: usize, find: &Find, theme: &Theme) {
        for (at, row) in rows.iter_mut().enumerate() {
            *row = find.paint(row, 0, find.current_unit() == Some(first + at), theme);
        }
    }

    fn apply(&mut self, action: NoKeys) -> PagerEffect<()> {
        match action {}
    }
}

let lines = (1..=40).map(|n| format!("line {n}")).collect();
let mut pager = Pager::new(Document(lines));
let theme = Theme::default();
let area = Rect::new(0, 0, 60, 12);
let mut buf = Buffer::empty(area);
pager.draw(area, &mut buf, &theme);

// `/`, a query, and `Enter`: the pager scrolls to the match, a row of
// context above it.
pager.handle_key(typed('/'));
for c in "line 30".chars() {
    pager.handle_key(typed(c));
}
pager.handle_key(press(KeyCode::Enter));
pager.draw(area, &mut buf, &theme);
assert!(rows(&buf).join("\n").contains("find \"line 30\" 1/1"));
assert_eq!(pager.window().offset(), 28);

// `Esc` clears the find, then leaves.
assert_eq!(pager.handle_key(press(KeyCode::Esc)), Outcome::Consumed);
assert_eq!(pager.handle_key(press(KeyCode::Esc)), Outcome::Close);
```

The same example is the doctest on the `pager` module, so it compiles.

### The content

`PagerContent` is what the host implements:

| Item | What it is | Default |
| --- | --- | --- |
| `type Message` | the host's message a request and the copy hook carry | (required) |
| `type Action` | the content's own key actions; `KeyAction + HelpAction` | (required; `NoKeys` for none) |
| `const KEYS` | the content's key table, resolved after the pager's; its footer segments go before the pager's leaving keys, its help sections before the pager's last | (required) |
| `layout(width, theme)` | lay the rows out for `width` cells and say how many; called by every draw, so keep the layout while `width` and the content's state are unchanged | (required) |
| `rows(range, theme)` | the rows in `range` of the last layout, styled | (required) |
| `header(width, theme)` | the header line (full screen) or title (framed) | (required) |
| `chips(theme)` | the state of the content's toggles, in order of importance | none |
| `find(query)` | the units that hold `query`, in order | none: the content does not find |
| `match_rows(unit)` | the rows a unit takes in the last layout | `unit..unit + 1` |
| `paint(rows, first, find, theme)` | pick out the active find's matches on the rows drawn | nothing |
| `find_detail(query)` | more the find chip says when nothing shown matches | none |
| `describe(key)` | the content's words for what a pager key does, on the help page (`Jump to the oldest line`) | the pager's words |
| `empty()` | what the body says with no rows | `nothing to show` |
| `apply(action)` | act on a content key; returns a `PagerEffect` | (required) |
| `copy()` | the copy hook: the message that copies what the content holds | none |

`PagerEffect` is `None` (the key is used), `Relayout` (the rows changed:
the pager runs the active find again, staying on the unit it was on, and
the next draw lays the rows out), `Close` (the pager closes, as `Esc` at
its first level does), or `Request(M)` (the host handles `M`, the pager
stays open). A host that changes the content itself (`content_mut`) calls
`refresh` afterwards, which does what `Relayout` does.

`scroll::WrappedRows` is the row counter most contents lay out with: it
wraps lines (`new`, or `hanging` under a gutter such as line numbers) or
cuts them at the edge with `…` (`cut`), and maps rows to lines and lines
to rows (`line_of`, `rows_of`), which is what `match_rows` and `paint` need.

### Modes

`PagerMode` says how the pager sits on the screen:

| Mode | Chrome | Position | Footer |
| --- | --- | --- | --- |
| `Screen` (the default) | the header, a rule, the body, a rule, a one-row footer; it fills the screen (`fills_screen`) | the `[ n/m ]` chip leading the footer: the first row shown and the rows in all | the position, the find chip, the note, the content's chips, then the keys, fitted to one row |
| `Modal(size)` | a `ModalFrame` of the named size, titled with the header | the shared scrollbar, in a column the rows give up when they overflow | the find chip and note (or the prompt), then the keys, receding and broken only between segments, at the body's text width, on the frame's last rows; a footer too tall keeps its last rows and scrolls the rest (below) |
| `Embedded` | none: the body in the area the host gives it | the shared scrollbar | none; the host routes the keys it wants (`scroll_by`, `page_down`, `to_end`) |

A full-screen footer too narrow for everything drops, in turn, the keys
that scroll, the pager's other keys (find, help), the content's chips
from the last, and the content's keys; the keys that leave stay, and what
is still too wide is cut with `…`. The header is cut with `…` too.

A framed pager leaves its body at least one row. When the prompt (or the
find chip and the note) and the footer need more rows than that leaves,
the prompt or chip row stays, the footer keeps its last rows (the keys
that leave), and its first rows (the keys that scroll) go after the
content's last row and scroll with it, so no key is lost and the keys
that leave are the last out of view, the same priority as the full-screen
footer's. The rows under the body are laid out at the body's text width,
a cell narrower beside the scrollbar when the body overflows. A frame
whose size fills its extent (`Fit::Fill`, as `WIDE` does) lays the
content out in the draw only.

### Scrolling

The pager keeps one window, `scroll::ScrollWindow`, and the offset asked
for. Each draw lays the content out, brings the offset inside it, and
keeps the window it showed, so:

- a page key moves by the rows the last draw showed, a half page by half
  of them (at least one), and `g`/`G` go to the ends;
- a key past the end is taken back at once: the next key up moves;
- keys pressed before the first draw count (`at_row`, `G`).

`G` (and `at_end` when the pager opens) makes the window keep to the end:
every draw shows the last page, so it follows the newest rows as the
content grows, the screen is resized, or a toggle lays the rows out again.
A key that moves the window (a scroll, a page, `g`, a find landing on a
match) lets go of the end.

### Find

`/` opens the find prompt on the footer row: a `text_edit::LineEdit`
holding the active query, with every text field's editing keys. `Enter`
confirms: the pager asks the content for the units that hold the query,
goes to the first one at or below the top of the window, and the next draw
always scrolls it to one row below the top (`CONTEXT_ROWS`), even when it
is already in view. An empty query clears the find. `Esc` leaves the
prompt and keeps the find that was active; `Ctrl-C` leaves the pager; `q`
types.

While a find is active:

- the footer's chip reads `find "q" 2/5`, or `find "q" no match` with the
  content's detail, the query cut to 17 cells;
- `n` and `N` step to the next and previous match, scrolling to each the
  same way, and wrap at the ends with a note (`wrapped to the first
  match`); with no match they say nothing, as the chip already does;
- the content paints its rows: `Find::paint` draws each match in the
  theme's match style, and the match the pager is on in the current-match
  look, the theme's `selection_active` modifier over the text.

`n` with no active find says `nothing to find yet: press / to search` in a
warning note. A host that opens the pager at a match found elsewhere calls
`land(query, unit)`, which goes to the match nearest `unit` and returns
whether there was one; `start_find(query)` is what confirming does.

### Levels and notes

`Esc` steps back one level at a time: the help page, then the prompt, then
the active find, then the pager itself (`Close`). `q` and `Ctrl-C` leave
from anywhere (`Dismiss`), except that `q` types in the prompt.

A note is a `feedback::Feedback` the footer shows until the next key: the
pager's own (a wrap, nothing to find) and the host's (`set_note`), for a
refusal or for what opening the pager changed. It is drawn as a status chip
in its severity's tone.

### The help page

`?` opens a `help::HelpPage` over the pager, generated from the pager's
table and the content's: the pager's sections (moving, finding), the
content's sections, then the pager's section of the keys that leave. The
page is read in a pager of its own over `PAGER_HELP_KEYS`, so it scrolls,
fits its footer, and shows its scrollbar as every framed pager does
(`help_page` is that pager while it is open). Its footer is `j/k/PgDn/PgUp
scroll · Esc/? back · q close`. While it is open the scroll keys move the
page, `Esc` and `?` (another `Close` there) go back, `q` and `Ctrl-C`
leave the pager, and every other key waits.

## Key table

The pager resolves `READER_KEYS` (a standard table; see
[keys.md](keys.md)) unless the host gives it another over `PagerKey` with
`Pager::keys` (a framed reader opened by a key closes on that key too, and
may bind no find or help). Then it resolves the content's `KEYS`. The help
level resolves `PAGER_HELP_KEYS` and the prompt `FIND_KEYS`.

| Key | Action | Notes |
| --- | --- | --- |
| `Ctrl-C` | Abort | leaves at once from any level (`Dismiss`); an alias |
| `j`, `↓` | Down | a row down |
| `k`, `↑` | Up | a row up |
| `PgDn`, `Space` | PageDown | the rows shown |
| `PgUp` | PageUp | the rows shown |
| `Ctrl-D` | HalfPageDown | half the rows shown, at least one; an alias |
| `Ctrl-U` | HalfPageUp | an alias |
| `g`, `Home` | Top | aliases |
| `G`, `End` | Bottom | the end, kept as the content grows; aliases |
| `Esc` | Close | steps back: the find, then the pager |
| `q` | Abort | leaves at once, a find active or not |
| `/` | Find | opens the prompt |
| `n` | FindNext | the next match, wrapping; an alias |
| `N` | FindPrev | the previous match, wrapping; an alias |
| `?` | Help | opens the help page |
| (a host's key) | Copy | the copy hook; no standard table binds it |
| any other key | (the content's, or none) | the content's `KEYS`, else swallowed |

The footer of `READER_KEYS` with a content key `x ask` reads
`j/k/PgDn/PgUp scroll · / find · ? help · x ask · Esc/q close`. The
prompt's footer, after the field, is `Enter find · Esc cancel`.

## Theme tokens

| Token | Used for |
| --- | --- |
| `inert` with `muted` | the rules, the separators, and the keys in a full-screen footer (`Tone::Inert`) |
| `muted` | a framed footer, and the empty note |
| `success` | the find chip with matches |
| `warning` | the find chip with no match, and a warning note |
| `info`, `error` | a note of those severities |
| `match_highlight` | every match the find is not on (`Theme::match_style`) |
| `selection_active` | the match the find is on, patched over the text |
| `field_cursor` | the prompt's caret (`text_edit::TextField`) |
| `accent`, `surface` | a framed pager's border, title, and background (`ModalFrame`), and the help page's key column |

The position chip and the find chip are bold; the content styles its own
rows. The scrollbar is the shared `ScrollGutter` look.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 2 | List ends | scrollable content clamps: every step stops at the ends, and the window is clamped on every draw |
| 4 | Close keys | `Esc` steps back one level (help, prompt, find, pager); `Ctrl-C` and `q` dismiss from any level; `q` types in the prompt |
| 5 | Close feedback | closing returns `Close` or `Dismiss` with nothing to report |
| 8 | Footers and hints | every footer (full screen, framed, help page, prompt) is generated from the tables in the one grammar |
| 11 | Feedback severity | a note is a `Feedback` drawn in its severity's tone: a wrap is info, a refusal a warning |
| 12 | Paging | viewport-sized `PgUp`/`PgDn`, `Home`/`End`/`g`/`G`, half-page `Ctrl-D`/`Ctrl-U`, and an offset clamped on every draw and kept |
| 13 | Theme | everything is drawn from theme tokens; the current match takes `selection_active` rather than a reversed cell of its own |
| 14 | Width | the header, the footer, and the find chip's query are cut in cells with `…`; contents lay out with `text` and `WrappedRows` |
| 15 | Text editing | the prompt is the one line editor and text field, with paste going to it (`takes_text`) |
| 18 | Pagers | `Esc` steps back (help, find, leave), `q` leaves, confirming a find always scrolls to the match, a `[ n/m ]` chip in Screen mode and a scrollbar in Modal and Embedded modes |

Not applicable: 1 and 10 (a pager has no cursor row; the match it is on
takes the selection look instead), 3 (nothing to confirm), 6 and 7 (no
form; a find with no match says so in its chip, not an error line), 9 (the
focus cue is the pane frame's), 16 (no path field), and 17 (the labels are
the host's).

## Seams left open

- **Anchors.** The window keeps its offset from the top, and to the end
  when `G` sticks it. A reader that keeps its place counted from the
  bottom as rows arrive (a live capture scrolled up a little) would keep
  the distance from the end instead; it is pager state, not content, so it
  can be added without changing `PagerContent`.
- **Sideways scrolling.** A content that does not wrap cuts its rows with
  `…`; a column offset (`h`/`l`) would show the rest of long rows.
- **Matches split by wrapping.** `Find::paint` paints a row at a time, so
  a match a wrap splits is found and stepped to but not painted.
- **Find scope.** The find is the content's: case-insensitive substring by
  default (`text::find_ci`), with no regular expressions or folding beyond
  ASCII.
- **Footer priorities.** Chips drop from the last; a content that wants
  some chip kept longest orders it first. A rank per chip would let it say
  so in place.
- **Copy of a selection.** The copy hook copies what the content holds; a
  copy of the rows shown or of the current match would pass the window or
  the find to the hook.
- **Mouse.** No mouse wheel; the scroll calls are public for a host that
  routes one.

## Ports

How each reader a host has moves onto the pager, as a check that the
content trait carries what each needs. Those marked built are; the rest
are sketches.

- **A snapshot of a terminal's scrollback** (built). `layout` numbers the
  lines in a five-cell gutter and wraps them under it
  (`WrappedRows::hanging`), or cuts them (`WrappedRows::cut`) when a `w`
  toggle (`Relayout`) says so; `find` lists the lines that hold the query
  and `paint` paints after the gutter, marking the current line's number;
  `header` names the source and counts the lines; `chips` shows the wrap
  toggle; one content key asks the host to attach (`Request`). The opener
  calls `land(query, line)` and, with no match, `set_note` and `at_row`.
- **A rich document of entries** (rendered from Markdown, with tables, and
  kinds of entry a toggle shows or hides). `layout` composes and caches
  the rows for the width and the toggles (the cache key is the width and
  the toggle states); `rows` slices the cache; `find` matches entries
  among the kinds the toggles show, and `find_detail` counts the matches
  in the hidden ones (`3 in hidden entries`); `match_rows` reads the start
  of each entry in the cache; `paint` paints after the entry's gutter,
  every row of the current entry in the current-match look; `chips` are
  the toggles' `ToggleChip`s, most important first; each toggle key
  returns `Relayout`, so the find stays on its entry. It opens `at_end`.
  Opening at a match found elsewhere: the host switches on the toggles
  that show the entry, calls `land(query, entry)`, and notes what it
  switched on. Host roles in its header (a source's color) are named
  styles read from the theme it is given, and `describe` keeps its own
  words for the ends and the find (`a match in hidden content turns that
  content on`).
- **A long value** (built; a framed reader opened by `o`).
  `Modal(ModalSize::WIDE)` with a table of `PAGER_KEYS`, `o` as another
  close, and `y` bound to `PagerKey::Copy`; `layout` wraps each line of the
  value at the width and keeps the rows while the width is the same,
  drawing a plan's command lines in the accent (the host's own hook, in
  its layout); `header` is the value's label; `copy` returns the host's
  copy message, carrying what to copy and the words its notice uses; no
  find and no help, since the table binds neither. The footer is the
  pager's order, `j/k/PgDn/PgUp scroll · y copy · Esc/q/o close`.
- **The help page** (built). `help::HelpPage` is itself the content:
  `layout` is the page's grid at the width, kept while the width is the
  same, and `header` its title (`Help`); it has no keys. A page opened by
  a key is `HelpPage::new(sections).pager(table)`, framed at
  `HelpPage::SIZE`, with the pager's keys and that key as another close
  (`PAGER_KEYS` and `?`): its footer, `j/k/PgDn/PgUp scroll · Esc/q/?
  close`, and its frames are the ones the page drew on its own. A
  pager's `?` level is the same page over `PAGER_HELP_KEYS`. A reader not
  yet on the pager keeps such a page and routes its own movement keys to
  the page's scroll calls.
- **A log's detail** (built; the framework's own `log_list::LogList`).
  `Embedded` under the log's list: `layout` wraps the selected entry's
  text a line at a time (`WrappedRows`); the list routes its detail keys
  (`J`/`K` to `scroll_by`, `PgDn` to `page_down`, `PgUp` to `page_up`),
  goes back to the top when the selection moves (`to_top`), and replaces
  the text when the selected entry's detail changes (`content_mut`,
  `refresh`). A footer too tall for the list's frame scrolls its first
  rows after the detail's last row, by the framed pager's rule.
- **A preview region.** `Embedded` under the host's banner. A source whose
  newest rows matter opens `at_end`; one showing a search's match calls
  `land` with the query; a source that cuts its own window lays out lazily,
  since `rows` asks only for the range drawn. Keys stay the host's.
