# `HelpPage`

| | |
| --- | --- |
| Module | `tuikit::help` |
| Layer | 2 (convention) |
| Type parameters | `A`: the action enum of the key table a page is generated from (it implements `HelpAction`); `M` (default `()`): the message type of the pager the page is read in, so the page can sit on a host's stack (the page sends none) |
| Cargo feature | none |
| Outcomes | none of its own: the page is a `PagerContent` with no keys, and its pager (`HelpPage::pager(table)`) answers `Close` for the table's closes, `Dismiss` for `q` and `Ctrl-C`, and `Consumed` for the rest |

## Purpose

`HelpPage` is the one help page. It lays out sections of keys and what
they do (the keys in an accent column, the descriptions wrapped beside
them) and legends for the marks a screen draws, and it is a pager's
content: `HelpPage::pager(table)` reads it in the framework's
[`Pager`](pager.md), framed at the reader size, which scrolls it as every
reader scrolls, draws the scrollbar when the page is taller than its
frame, and generates the footer from the key table that scrolls and
closes it. `sections_for` generates the sections from a key table and a
description for each action, so a page cannot disagree with the keys it
describes, and `undescribed` finds a key the page would leave out.

The module replaces what a program grows help page by help page: a
generator copied for every viewer that differs only in its message type,
hand-written key strings and an inert binding struct kept apart from the
key tables, a page renderer that each page reached into for its row
helpers, pages that stepped by a fixed number of rows or by the height of
the screen behind them, scroll offsets that kept growing past the end so
the next key up did nothing, and a scroll state and renderer of the
page's own beside the readers' pager.

The host keeps what each action does and where it is listed (its
`HelpAction` impl), the prose of the rows it writes by hand where a table's
one-line descriptions are not enough, the marks its legends explain and
their styles, which table scrolls and closes its pages, and which actions
are available now.

## Usage example

A reader's page generated from its key table, read in its pager, and a
hand-written page with a legend:

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use tuikit::help::{HelpAction, HelpEntry, HelpPage, HelpSection, sections_for, undescribed};
use tuikit::keys::{
    KeyRow, PAGER_KEYS, PagerKey, alias, ch, ctrl, include, key, shown, with_availability,
};
use tuikit::overlay::Outcome;
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

// The reader's page: one row per action, every key bound to it, read in
// a pager whose table scrolls and closes it and generates its footer.
assert!(undescribed(READER_KEYS).is_empty(), "every key is described");
let mut page = HelpPage::<()>::new(sections_for(READER_KEYS)).pager(HELP_KEYS);
let mut buf = Buffer::empty(area);
page.draw(area, &mut buf, &theme);

// The page's keys are its pager's: `G` goes to the last page, `?` closes.
let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
assert_eq!(page.handle_key(press(KeyCode::Char('G'))), Outcome::Consumed);
assert_eq!(page.handle_key(press(KeyCode::Char('?'))), Outcome::Close);

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
let lines = HelpPage::<()>::new(marks.to_vec()).lines(&theme, 76);
assert_eq!(lines.len(), 7, "two headings, three rows, two blank lines");
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
| `HelpPage::new(sections)`, `title` | a page of `sections`, titled `Help` unless `title` says otherwise | `HelpPage::SIZE` is `ModalSize::WIDE` |
| `HelpPage::pager(table)` | the page in a `Pager` framed at `SIZE`, resolving `table`, the keys that scroll and close it | the footer is generated from `table`, so it cannot name a key the table does not bind |
| `lines(theme, width)`, `sections()` | the page laid out in cells, as its pager draws its rows; the sections | `usize::MAX` wraps nothing |
| `PagerContent for HelpPage<M>` | rows laid out once per width, the title as the header, no keys (`NoKeys`) | the pager does the scrolling, the scrollbar, and the footer |

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
- The footer is the pager's: muted, wrapped only between its segments,
  and on the frame's last rows whatever the scroll, leaving the page at
  least one row; when it does not fit, its last rows (the keys that leave)
  stay and its first rows scroll with the page, after its last row, the
  rule every framed pager follows.
- When the page is taller than the frame, the pager's scrollbar takes a
  1-column gutter on the right and the text is laid out one cell
  narrower.

### Scrolling

The pager's (see [pager.md](pager.md)): one window, clamped on every draw
and kept, so after any number of keys past the end the next key up
scrolls up; a page key moves by the rows the page showed, a half-page key
by half of them (at least one), never by the height of a screen behind
the page; `G` (pressed before the first draw too) shows the last page and
keeps to it.

## Key table

The page binds no keys itself. Its pager resolves the table it was given
(`HelpPage::pager(table)`), the keys that scroll and close it, and the same
table generates the footer. A help page opened by a key is the standard
pager plus that key:

| Key | Action | Notes |
| --- | --- | --- |
| `j` / `↓`, `k` / `↑` | scroll a row | `PAGER_KEYS` |
| `PgDn` / `Space`, `PgUp` | scroll a page | `PAGER_KEYS`; a page is the rows shown |
| `Ctrl-D`, `Ctrl-U` | scroll half a page | `PAGER_KEYS` (aliases) |
| `g` / `Home`, `G` / `End` | the top, the end | `PAGER_KEYS` |
| `Esc`, `q`, `Ctrl-C` | close | `PAGER_KEYS` (`Ctrl-C` first, as an alias; `q` with it as `Abort`) |
| the key that opened it (`?`) | close | the host's row, `shown(ch('?'), PagerKey::Close, "?", "close")` |

A page that is a level of a reader (the `Pager`'s `?` page) is read in a
pager over `PAGER_HELP_KEYS` instead: the same scrolling, `Esc` and `?`
closing it, back to the reader, and `q` leaving the reader, with the
footer `j/k/PgDn/PgUp scroll · Esc/? back · q close`. A control with its
own keys maps its movement actions onto the page's pager's scroll calls
(`scroll_by`, `page_down`, `half_page_up`, `to_end`).

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
- **Hiding instead of dimming.** An unavailable row is dimmed; hiding it
  is a filter over the sections, and explaining it is the control's.
- **Per-section ordering and columns.** The key column is one width for
  every page so pages line up; a page with only short keys could narrow
  it, at the cost of the shared grid.
- **Search within the page.** The page finds nothing (its tables bind no
  find); a `find` over the laid-out lines and a table that binds `/` would
  give it the pager's.
