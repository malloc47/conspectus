# `HelpPage`

| | |
| --- | --- |
| Module | `tuikit::help` |
| Layer | 2 (convention) |
| Type parameters | `A`: the action enum of the key table a page is generated from (it implements `HelpAction`); the key table that scrolls and closes the page is any table, over its own enum |
| Cargo feature | none |
| Outcomes | none of its own: the page draws and returns the `ScrollWindow` it showed; the control that shows it resolves its keys (usually through a pager table) and moves its `HelpScroll` |

## Purpose

`HelpPage` is the one help page. It draws sections of keys and what they
do (the keys in an accent column, the descriptions wrapped beside them),
legends for the marks a screen draws, a scrollbar when the page is taller
than its frame, and a footer generated from the key table that scrolls and
closes it, in the reader-sized modal frame. `sections_for` generates the
sections from a key table and a description for each action, so a page
cannot disagree with the keys it describes, and `undescribed` finds a key
the page would leave out. `HelpScroll` keeps where the page is: a key may
ask past the end, the draw brings it back, and a page key moves by the
rows on screen.

The module replaces what a program grows help page by help page: a
generator copied for every viewer that differs only in its message type,
hand-written key strings and an inert binding struct kept apart from the
key tables, a page renderer that each page reached into for its row
helpers, pages that stepped by a fixed number of rows or by the height of
the screen behind them, and scroll offsets that kept growing past the end
so the next key up did nothing.

The host keeps what each action does and where it is listed (its
`HelpAction` impl), the prose of the rows it writes by hand where a table's
one-line descriptions are not enough, the marks its legends explain and
their styles, which table scrolls and closes its pages, and which actions
are available now.

## Usage example

A reader's page generated from its key table, a hand-written page with a
legend, and the scroll kept between draws:

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use tuikit::help::{
    HelpAction, HelpEntry, HelpPage, HelpScroll, HelpSection, sections_for, undescribed,
};
use tuikit::keys::{
    KeyRow, PAGER_KEYS, PagerKey, alias, ch, ctrl, include, key, resolve, shown,
    with_availability,
};
use tuikit::theme::Theme;

/// What a document reader's keys do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reader {
    Down,
    Up,
    Help,
    Leave,
}

impl HelpAction for Reader {
    const LISTED: &'static [Self] = &[Reader::Down, Reader::Up, Reader::Help, Reader::Leave];

    // An exhaustive match: a new action fails to compile until described.
    fn help(self) -> HelpEntry {
        match self {
            Reader::Down => HelpEntry::new("Moving", "Scroll one line down"),
            Reader::Up => HelpEntry::new("Moving", "Scroll one line up"),
            Reader::Help => HelpEntry::new("Reader", "Show or hide this help"),
            Reader::Leave => HelpEntry::new("Reader", "Close the reader"),
        }
    }
}

const READER_KEYS: &[KeyRow<Reader>] = &[
    alias(ctrl('c'), Reader::Leave),
    alias(ch('j'), Reader::Down),
    alias(key(KeyCode::Down), Reader::Down),
    alias(ch('k'), Reader::Up),
    alias(ch('?'), Reader::Help),
    alias(ch('q'), Reader::Leave),
];

/// The keys that scroll and close a help page opened with `?`.
const HELP_KEYS: &[KeyRow<PagerKey>] = &[
    include(PAGER_KEYS),
    shown(ch('?'), PagerKey::Close, "?", "close"),
];

let theme = Theme::default();
let area = Rect::new(0, 0, 80, 24);

// The reader's page: one row per action, every key bound to it.
assert!(undescribed(READER_KEYS).is_empty(), "every key is described");
let sections = sections_for(READER_KEYS);
let mut scroll = HelpScroll::new();
let mut buf = Buffer::empty(area);
let window = HelpPage::new(&theme, &sections)
    .footer(HELP_KEYS)
    .scrolled(&scroll)
    .render(area, &mut buf);
scroll.keep(window);

// The page's keys: a pager's, moving the scroll the host keeps.
let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
match resolve(HELP_KEYS, press(KeyCode::Char('G'))) {
    Some(PagerKey::Abort | PagerKey::Close) => { /* close the page */ }
    Some(step) => {
        scroll.apply(step);
    }
    None => {}
}

// An action that cannot act now is dimmed on the page.
let dimmed = with_availability(READER_KEYS, |action| action != Reader::Up);
let _ = sections_for(&dimmed);

// A page written by hand: prose rows, and a legend of the marks a screen
// draws, each mark in its own style.
let marks = [
    HelpSection::new("Editing")
        .keys("Enter", "Open the row; a group row opens or closes"),
    HelpSection::new("Marks")
        .legend(Span::styled("*", Style::default().fg(theme.warning)), "Changed", "edited since it was opened")
        .legend(Span::styled("+", Style::default().fg(theme.success)), "Added", "new since it was opened"),
];
let mut buf = Buffer::empty(area);
HelpPage::new(&theme, &marks).footer(HELP_KEYS).render(area, &mut buf);
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `HelpAction` | an action enum a page can be generated for | `LISTED` (the page's order) and `help` (an exhaustive match giving each action its `HelpEntry`) |
| `HelpEntry::new(section, description)` | where an action is listed and what it does | actions naming one heading share a section |
| `sections_for(table)` | the page of a key table | one row per listed action the table binds; rows group under their sections in listing order; an unbound action is left out |
| `undescribed(table)` | the keys a table binds that its page would not describe | an action left out of `LISTED`, or one whose entry is empty; a host asserts it is empty for every table it generates a page for |
| `HelpSection::new(title)`, `keys`, `legend`, `row` | a heading and its rows, built by hand | for prose a table's descriptions cannot hold |
| `HelpRow::Keys`, `HelpRow::Legend` | a row of keys and what they do; a legend entry | `HelpRow::keys(..).available(false)` dims a row |
| `HelpPage::new(&theme, &sections)` | a page titled `Help`, at the top, with no footer | `SIZE` is `ModalSize::WIDE` |
| `title`, `footer(table)`, `scrolled(&scroll)` | set the title, generate the footer from the keys that scroll and close the page, scroll it | the footer cannot name a key the table does not bind |
| `lines(width)`, `footer_lines(width)` | the page laid out in cells, above and in its footer | what `render` draws; `usize::MAX` wraps nothing |
| `render(area, buf)` | draws the page in its frame | returns the `ScrollWindow` its scrolling part showed |
| `HelpScroll` | where a page is scrolled, kept between draws | `down`, `up`, `page_down`, `page_up`, `half_page_down`, `half_page_up`, `to_top`, `to_bottom`, `apply(PagerKey)`, `keep(window)`, `offset`, `page` |

### Generated rows

A generated row lists every key the table binds to its action, included
tables spliced in, written by `key_label` (`↑`, `PgDn`, `Ctrl-D`), plain
keys before Control and Alt chords and otherwise in table order, joined
with ` / ` (`q / Ctrl-C`). Aliases are listed like shown keys: a footer
shows one key of each action, and the help lists them all. A row is
available when any of its keys is, so a table copied through
`with_availability` dims the rows of actions that cannot act now, the key
column included. An action listed twice gets one row.

### Layout

Every width follows one grid, so every help page of a program lines up:

- A heading is bold and word-wrapped, followed by its rows and a blank
  line.
- A key row is indented two cells; the keys, in `accent`, fill a 16-cell
  column, and the description starts at its end, wrapped under a hanging
  indent there. Keys that leave no space before the column's end take a
  line of their own, with the description under them.
- A legend row is indented two cells; the mark, in its own style, fills
  the section's mark column (the widest mark and a space), and the name,
  bold, fills the section's name column (the longest name and a space,
  at least 16 cells, so short-named legends line up). The blurb wraps
  under a hanging indent at the name column's end.
- Below 25 cells beside its column, a description stays on one row and
  the frame clips it.
- The footer is muted and wraps only between its segments. It stays on
  the frame's last rows whatever the scroll and leaves the page at least
  one row; footer rows that do not fit there scroll with the page.
- When the page is taller than the frame, a scrollbar takes a 1-column
  gutter on the right and the text is laid out one cell narrower.

### Scrolling

`HelpScroll` holds the offset the page asks for and the rows the last draw
showed. A step may carry the offset past the end (so `G` works before the
first draw); each draw shows the page at the offset brought inside it and
returns that window, and `keep` takes it back, so after any number of keys
past the end the next key up scrolls up. A page key moves by the rows the
page showed, a half-page key by half of them (at least one), never by the
height of a screen behind the page.

## Key table

The page binds no keys itself. The control that shows it resolves the keys
that scroll and close it, and the same table generates the footer. A help
page opened by a key is the standard pager plus that key:

| Key | Action | Notes |
| --- | --- | --- |
| `j` / `↓`, `k` / `↑` | scroll a row | `PAGER_KEYS`; `HelpScroll::apply(PagerKey::Down)` |
| `PgDn` / `Space`, `PgUp` | scroll a page | `PAGER_KEYS`; a page is the rows shown |
| `Ctrl-D`, `Ctrl-U` | scroll half a page | `PAGER_KEYS` (aliases); `HelpScroll::apply(PagerKey::HalfPageDown)` |
| `g` / `Home`, `G` / `End` | the top, the end | `PAGER_KEYS` |
| `Esc`, `q`, `Ctrl-C` | close | `PAGER_KEYS` (`Ctrl-C` first, as an alias; `q` with it as `Abort`) |
| the key that opened it (`?`) | close | the host's row, `shown(ch('?'), PagerKey::Close, "?", "close")` |

A page that is a level of a reader (the `Pager`'s `?` page) resolves
`PAGER_HELP_KEYS` instead: the same scrolling, `Esc` and `?` going back to
the reader, and `q` leaving it, with the footer `j/k/PgDn/PgUp scroll ·
Esc/? back · q close`. A control with its own keys maps its movement
actions onto the same `HelpScroll` steps, half pages included.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | the key column, and the frame's border and title |
| `muted` | the footer |
| `inert` | an unavailable row and footer segment (`unavailable_style`, with `muted`) |
| `surface` | the frame's background |

A heading and a legend name are bold. A legend mark keeps the style its
host gave it: the framework names no color for it.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 2 | List ends | A page is scrollable content: it clamps at both ends |
| 4 | Close keys | Closing is the table's: the standard pager rows close on `Esc`, `q`, and `Ctrl-C`, and a page opened by a key closes on it too |
| 8 | Footers and hints | The footer is generated from the key table, and a generated page from the key table it describes, so neither can name a key the control does not act on |
| 12 | Paging | A page key moves by the rows the page showed; a half page by half of them; an offset past the end comes back at the next draw |
| 13 | Theme | Keys in `accent`, the footer `muted`, an unavailable row from `inert` and `muted`; marks keep the host's styles |
| 14 | Width | Headings, descriptions, and blurbs wrap in cells through the `text` module, keys pad in cells, and no row is wider than the page |

Not applicable: 1 and 10 (a page has no cursor), 3 (it confirms nothing),
5 (closing posts nothing; the stack and the host decide), 6, 7, 15, and
16 (no fields, no editing), 9 (the frame is a modal's, not a pane's), 11
(no feedback), 17 (labels are the host's copy), and 18 (a pager's find and
stepping are the pager's; a page inside a pager steps back first, which
the pager decides).

## Seams left open

- **A program-wide keymap's page.** `sections_for` reads any table of
  `KeyRow`s; the scoped program-wide keymap planned for `keys` carries
  help text on its rows and generates the main page, with hand-written
  rows kept as annotations beside the generated ones.
- **The pager.** The page's body is a reader that keeps its window; a
  pager control with a modal mode can take it over, keeping the layout and
  the generator here.
- **Hiding instead of dimming.** An unavailable row is dimmed; hiding it
  is a filter over the sections, and explaining it is the control's.
- **Per-section ordering and columns.** The key column is one width for
  every page so pages line up; a page with only short keys could narrow
  it, at the cost of the shared grid.
- **Search within the page.** A find over the laid-out lines would sit
  with the pager's find.
