# `LogList<R, M, Cx>`

| | |
| --- | --- |
| Module | `tuikit::log_list` |
| Layer | 1 (control) |
| Type parameters | `R`, the payload of the host's log entries (`feedback::Entry<R>`: what each was about, what ran, what it printed); `M`, the host's message, which the copy hook builds; `Cx`, the host's context, from which the list reads the log and the time on every key and every draw. The list implements `Overlay<M, Cx>` for that context |
| Cargo feature | none |
| Outcomes | `Consumed` for a key it used or swallowed; `Close` for `Esc` (and a host's opening key bound to `Close`); `Dismiss` for `q` and `Ctrl-C`; `Stay(M)` for the copy hook's message |

## Purpose

`LogList` is the reader of a program's log: what its operations did, newest
first, with the selected entry's detail (the output it printed, the command
that ran) under the list. Without it each program writes its own: one wraps
at the ends of the list while another stops, one pages the detail by a fixed
eight rows and forgets to clamp it, one draws its own selection look, and
each writes its footer by hand.

The model is the framework's `feedback::Log<R>` (bounded, newest last,
repeats collapsed, warnings and errors counted until seen); the list reads
it from the host's context. The host keeps the payload type and what it
means: how an entry's detail reads, what a copy copies and the message that
asks for it, the title, the words the empty list says, the clock the ages
are read against, and the keys it adds (a copy key, the key that opened the
list).

## Usage example

A program whose log entries carry what each operation printed. The list
opens over the log (seeing it), selects the older entry, asks the host to
copy it, draws, and closes.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use tuikit::feedback::{Entry, Log, Severity};
use tuikit::keys::{KeyRow, LOG_LIST_KEYS, LogListKey, ch, include, shown};
use tuikit::log_list::LogList;
use tuikit::overlay::{Outcome, Overlay};
use tuikit::testkit::buffer::rows;
use tuikit::testkit::keys::press;
use tuikit::theme::Theme;

/// The program's state: its log, each entry carrying what it printed.
struct State {
    log: Log<String>,
    now: i64,
}

#[derive(Debug, PartialEq)]
enum Msg {
    Copy(String),
}

const KEYS: &[KeyRow<LogListKey>] = &[
    include(LOG_LIST_KEYS),
    shown(ch('y'), LogListKey::Copy, "y", "copy"),
];

let mut state = State { log: Log::new(100), now: 1_060 };
state.log.push(Entry::new(1_000, Severity::Error, "export failed", "disk full".into()));
state.log.push(Entry::new(1_030, Severity::Success, "saved", "3 files".into()));

let mut list = LogList::new("Events", |state: &State| &state.log, |state: &State| state.now)
    .keys(KEYS)
    .detail(|entry: &Entry<String>| format!("{}\n{}", entry.summary, entry.item))
    .copy(|entry: &Entry<String>| Some(Msg::Copy(entry.item.clone())))
    .mark_seen(&mut state.log);
assert_eq!(state.log.unseen(), 0, "opening the list saw the failure");

// `j` selects the older entry; `y` asks the host to copy it.
assert_eq!(list.handle(press(KeyCode::Char('j')), &state), Outcome::Consumed);
assert_eq!(
    list.handle(press(KeyCode::Char('y')), &state),
    Outcome::Stay(Msg::Copy("disk full".into()))
);

let theme = Theme::default();
let mut buf = Buffer::empty(Rect::new(0, 0, 60, 14));
list.render(buf.area, &mut buf, &state, &theme);
let drawn = rows(&buf).join("\n");
assert!(drawn.contains("Events · 2"));
assert!(drawn.contains("► ×   1m ago  export failed"));
assert!(drawn.contains("disk full"), "the detail shows the selection");
assert!(drawn.contains("j/k select · J/K scroll detail · y copy · Esc close"));

// `Esc` closes it.
assert_eq!(list.handle(press(KeyCode::Esc), &state), Outcome::Close);
```

The same example is the doctest on the `log_list` module, so it compiles.

### The API

| Item | What it is |
| --- | --- |
| `LogList::new(title, source, clock)` | a list titled `title` over the log `source` finds in the context (`Fn(&Cx) -> &Log<R>`), with ages read against `clock` (`Fn(&Cx) -> i64`, Unix seconds) |
| `.keys(table)` | resolve keys with a host table that includes `LOG_LIST_KEYS` and adds its own rows; the footer is generated from it |
| `.detail(f)` | how an entry's detail reads (`Fn(&Entry<R>) -> String`); the summary with its repeat count until set |
| `.copy(f)` | the copy hook (`Fn(&Entry<R>) -> Option<M>`), asked for the selected entry by a key bound to `LogListKey::Copy` |
| `.empty(note)` | what the list says while the log is empty; `nothing logged yet` until set |
| `.mark_seen(&mut log)` | the unseen-count hook: marks the log seen as the list opens over it, so a count the host shows elsewhere (a status chip) clears |
| `selected()`, `selected_entry(&log)` | the selection, newest first (0 is the newest) |
| `window()`, `detail_window()` | the list's and the detail's scroll windows at the last draw |
| `footer()`, `title()`, `table()` | the generated footer, the title, and the table |
| `draw(area, buf, cx, theme)` | draw it; `Overlay::render` does this on a stack |

`LOG_LIST_SIZE` is the frame it draws in: `ModalSize::WIDE`, the screen
less its margins.

### Layout

The frame is titled with the title and the count of entries (`Events ·
12`). Inside it, from the top:

- **The entries**, up to two fifths of the rows (at least three while the
  log has three). Each row is the cursor gutter, the severity's mark in its
  theme color, the age right-aligned in the age cell's width with its prose
  suffix (`  3m ago`), and the summary with its repeat count (`refresh
  failed (×3)`, `Entry::summary_with_repeats`), cut with `…` to fit. The
  rows scroll in a window the list keeps, which moves no further than the
  selection needs, with the shared scrollbar when they overflow.
- **A rule** across the frame.
- **The detail** of the selected entry: an embedded `Pager` over the host's
  text, a line of the text at a time, wrapped by the framework's rules (a
  cell narrower beside its scrollbar when it overflows). It keeps its own
  window, clamped on every draw, and goes back to its top when the
  selection moves; an entry logged while the list is open shows at the next
  draw.
- **The footer** on the frame's last rows, generated from the table,
  drawn by the frame's `ModalFooter` ([frame.md](frame.md#the-footer)):
  receding in `muted`, under a blank row, broken only between segments.
  When the footer is taller than the room under the rule leaves beside a
  row of detail, its blank row goes first, its last rows (the keys that
  leave) stay, and its first rows go after the detail's last row, behind a
  blank row, and scroll with it, by the one fit every framed control
  shares, so no key is lost.

An empty log shows the host's note, each line wrapped at the frame's width,
over the footer and its blank row; the note keeps a row on the shortest
frames.

## Key table

The list resolves `LOG_LIST_KEYS` (a standard table; see [keys.md](keys.md))
unless the host gives it another that includes it. No standard row binds
`LogListKey::Copy`; a host whose entries copy binds it (`y`), and a list
opened by a key adds that key as another `Close`.

| Key | Action | Notes |
| --- | --- | --- |
| `Ctrl-C` | Abort | dismisses the list (`Dismiss`); an alias |
| `q` | Abort | an alias: nothing is typed in a log list |
| `j`, `↓`, `Ctrl-N` | Down | the next entry, older; stops at the oldest |
| `k`, `↑`, `Ctrl-P` | Up | the next entry, newer; stops at the newest |
| `g`, `Home` | Newest | aliases |
| `G`, `End` | Oldest | aliases |
| `J` | DetailDown | the detail a row down |
| `K` | DetailUp | the detail a row up |
| `PgDn`, `Space` | DetailPageDown | the detail a page down: the rows it shows; aliases |
| `PgUp` | DetailPageUp | an alias |
| `Esc` | Close | closes the list |
| (a host's key) | Copy | the copy hook's message, with `Stay` |
| any other key | (none) | swallowed |

The footer is the keys that select and scroll, the list's other keys, then
the keys that leave: `j/k select · J/K scroll detail · Esc close` for the
standard table, and `j/k select · J/K scroll detail · y copy · Esc close`
with a copy key.

## Theme tokens

| Token | Used for |
| --- | --- |
| `info`, `success`, `warning`, `error` | each entry's severity mark (`Severity::style`) |
| `inert` | the age |
| `muted` | the rule under the list and the footer |
| `selection_active` | the selected row, from the gutter to its last cell (`selection::row`) |
| `accent`, `surface` | the frame's border, title, and background (`ModalFrame`) |

The scrollbars are the shared `ScrollGutter` look. The detail's text is
unstyled: the host's words.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | the selected row is drawn through `selection::row`: the pointer in the 2-cell gutter and the `selection_active` highlight |
| 2 | List ends | a log is scrollable content: the selection stops at the newest and the oldest entry (`Ends::Clamp`), and the detail at its ends |
| 4 | Close keys | `Esc` closes; `Ctrl-C` and `q` dismiss |
| 5 | Close feedback | closing returns `Close` or `Dismiss` with nothing to report |
| 8 | Footers and hints | the footer is generated from the table, the keys that leave last, and drawn by `ModalFooter` in the one look and fit |
| 11 | Feedback severity | each entry's mark and its color come from its severity and the theme |
| 12 | Paging | the detail pages by the rows it shows and its window is clamped on every draw; `g`/`G` and `Home`/`End` go to the ends of the list |
| 13 | Theme | every style is a theme token; no raw color |
| 14 | Width | rows are cut in cells with `…`; the detail and the empty note wrap by the `text` rules |
| 18 | Pagers | the detail is the framework's pager in its embedded mode, with the shared scrollbar |

Not applicable: 3 (nothing to confirm), 6 and 7 (no form), 9 (the focus
cue is a pane's), 10 (the list always has the keys, so its selection is
always the active look), 15 and 16 (nothing is typed), and 17 (the labels
are the host's).

## Seams left open

- **Half pages in the detail.** The detail is a pager, which has
  `half_page_down`; no row of `LOG_LIST_KEYS` binds `Ctrl-D`/`Ctrl-U` to
  it yet, so the list keeps the keys it replaced.
- **A selection that follows its entry.** The selection is a position,
  newest first; an entry logged while the list is open moves the selected
  entry down a row. Keeping the entry would need an identity on `Entry`.
- **Styled detail.** The detail is plain text; a hook returning styled
  lines (a failed command's stderr in the error color) would replace the
  `String` with lines.
- **Finding in the log.** The detail pager binds no find; a find over the
  entries (their summaries and details) would be a palette over the log or
  the pager's find over the detail.
- **Unseen entries marked in the list.** The log counts unseen entries but
  not which; marking them would need the log to keep where it was last
  seen.
- **Mouse.** No wheel or click; the scroll calls are the pager's and the
  window the list's.
