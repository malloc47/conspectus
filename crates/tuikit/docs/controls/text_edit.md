# `LineEdit` and `TextField`

| | |
| --- | --- |
| Module | `tuikit::text_edit` |
| Layer | 1 (control) |
| Type parameters | none |
| Cargo feature | none |
| Outcomes | `LineEdit::handle` returns an `Edit`: `consumed` (the key was an editing key or typed a character) and `changed` (the text changed); `TextField` only draws |

## Purpose

`LineEdit` is the one line editor: a line of text and a cursor, edited by
one table of keys. `TextField` is the one way to draw it: in a single row,
windowed in terminal cells so the cursor stays in view, with a caret from
the theme. Every text field a program has (a prompt, a form's text row, a
filter query, a find prompt) uses the two, so a key works the same in each
and a wide character puts the caret on the right cell in each.

They replace what a program grows field by field: a second, hand-rolled
editor with its own key subset, a windowing loop per renderer (counting
characters in some and cells in others), a query line drawn as plain text
with no caret, and a copy of the value before every key to learn whether
the key changed it.

The editing engine underneath is `tui-input`, which owns graphemes, word
boundaries, and the kill buffer. `LineEdit` drives it with its own key
table, so the word keys answer to Alt, which is what a terminal reports
(the engine binds them to Meta).

The host keeps what the field is for: its label or prompt, what `Enter`,
`Esc`, and `Tab` do (submit, cancel, complete, move to the next field),
what a change triggers (a refilter, a validation, clearing an error), and
whether a paste reaches the field (the overlay contract's `takes_text`).

## Usage example

A filter query in a small host: `Enter` and `Esc` are the host's, every
other key goes to the editor, and the list refilters only when the text
changed. The query draws after a `/` with its caret.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use tuikit::text_edit::{LineEdit, TextField};
use tuikit::theme::Theme;

enum Msg {
    Pick(String),
    Close,
}

struct Filter {
    query: LineEdit,
    items: Vec<&'static str>,
    shown: Vec<&'static str>,
}

impl Filter {
    fn handle(&mut self, key: KeyEvent) -> Option<Msg> {
        match key.code {
            KeyCode::Enter => return self.shown.first().map(|item| Msg::Pick(item.to_string())),
            KeyCode::Esc => return Some(Msg::Close),
            _ => {}
        }
        if self.query.handle(key).changed {
            let needle = self.query.value().to_lowercase();
            self.shown = self.items.iter().copied().filter(|item| item.contains(&needle)).collect();
        }
        None
    }

    fn draw(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        TextField::new(&self.query)
            .prefix(Span::styled("/", Style::new().fg(theme.accent)))
            .render(Rect { height: 1, ..area }, buf, theme);
    }
}

let theme = Theme::default();
let mut filter = Filter {
    query: LineEdit::default(),
    items: vec!["alpha", "beta", "alphabet"],
    shown: Vec::new(),
};
for c in "alp".chars() {
    filter.handle(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
}
assert_eq!(filter.shown, ["alpha", "alphabet"]);

let area = Rect::new(0, 0, 20, 1);
let mut buf = Buffer::empty(area);
filter.draw(area, &mut buf, &theme);
assert_eq!(buf[(0, 0)].symbol(), "/");
assert_eq!(buf[(4, 0)].bg, ratatui::style::Color::White, "the caret after `alp`");
```

A host that turns keys into messages asks `EditKey::of(key)` for the edit
and carries it in its message, then applies it with `LineEdit::apply`. A
host that types a bracketed paste one key at a time flattens it with
`single_line_paste` first; `LineEdit::paste` does both at once.

A form's text row is the same field filled and marked:

```rust
use tuikit::text_edit::{LineEdit, TextField};
use tuikit::theme::Theme;

let theme = Theme::default();
let name = LineEdit::new("a value longer than its field");
let spans = TextField::new(&name)
    .focused(true)
    .marks(true)
    .fill(true)
    .placeholder("-")
    .style(theme.field_active.into_style())
    .spans(16, &theme);
let text: String = spans.iter().map(|span| span.content.as_ref()).collect();
assert_eq!(text, "< n its field   ", "scrolled to the cursor, with the `<` mark");
```

## Key table

`LINE_EDIT_KEYS`, over `EditKey`. Every row is an alias: a field's footer
names its host's keys (`Enter`, `Esc`), not the editing keys every field
shares. A host consults this table after its own, so a key the host binds
(a form's `←`/`→` on a choice row, a search's `Ctrl-R`) never reaches the
editor. A character typed alone, with Shift, or with Control and Alt (AltGr)
types itself (`EditKey::Insert`).

| Key | Action | Notes |
| --- | --- | --- |
| `←`, `Ctrl-B` | Left | one grapheme; Shift is ignored |
| `→`, `Ctrl-F` | Right | one grapheme |
| `Alt-b`, `Ctrl-←`, `Alt-←` | WordLeft | to the start of the word before the cursor |
| `Alt-f`, `Ctrl-→`, `Alt-→` | WordRight | to the start of the next word, or the end |
| `Home`, `Ctrl-A` | Home | start of the line |
| `End`, `Ctrl-E` | End | end of the line |
| `Backspace`, `Ctrl-H` | DeleteBefore | the grapheme before the cursor |
| `Delete` | DeleteAfter | the grapheme under the cursor |
| `Ctrl-W`, `Alt-Backspace` | DeleteWordBefore | back to the start of the word before |
| `Alt-d`, `Ctrl-Delete` | DeleteWordAfter | forward to the start of the next word |
| `Ctrl-U` | DeleteLine | the whole line, wherever the cursor is |
| `Ctrl-K` | DeleteToEnd | from the cursor to the end; nothing at the end |
| `Ctrl-Y` | Yank | types what the last word, line, or rest-of-line deletion removed |

`Enter`, `Esc`, `Tab`, `S-Tab`, `↑`, `↓`, the page keys, and every other
chord are not consumed: `handle` returns `Edit::IGNORED` and the host acts
on them. The prompt contract (`Enter` confirm, `Esc` cancel, `Ctrl-C`
abort, `Tab` swallowed) is the standard `TEXT_INPUT_KEYS` table (see
[keys.md](keys.md)).

## Theme tokens

| Token | Used for |
| --- | --- |
| `field_cursor` | the caret: the cell under the cursor while the field is focused |
| `accent` | the `<` and `>` scroll marks (bold, over the field's style) |

The field's text, padding, and the ground of its marks take the style the
host passes (`TextField::style`); a form passes `field_active` for the
focused field and `field` for the others, and a query line passes none. The
prefix carries its own style. An unfocused field draws no caret: the cell
under its cursor takes the field's style like the rest of the text.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 4 | Close keys | the editor binds no `Esc`, `Ctrl-C`, or `q`, so leaving is always the host's; `q` types |
| 8 | Footers and hints | the editing keys are a key table of aliases, so no footer advertises them and none is hand-written |
| 13 | Theme | the caret is `field_cursor` and the marks `accent`; no raw color or reversed cell |
| 14 | Width | the window, the marks, the fill, and the placeholder are measured in cells and move in whole graphemes; a wide character never straddles the edge |
| 15 | Text editing | one engine for every field: cell-width-correct editing and windowing, word motion on Alt and Control, and the one-line paste rule (`single_line_paste`) |

Not applicable: 1, 2, 10, and 12 (the field is not a list and does not
page), 3 and 5 (it confirms and cancels nothing), 6 and 16 (the form's
ring and path fields are `Form`'s and `PathField`'s), 7 (validation and
its error line belong to the prompt or form around the field), 9 (the
focus cue is the pane frame's; a field shows focus with its caret), 11
(it gives no feedback), 17 (labels are the host's copy), and 18 (pagers).

## Seams left open

- **The terminal cursor.** The caret is a styled cell. A host that wants
  the terminal's own cursor (for an input method, or a blinking caret)
  needs the caret's column, which the window already computes and could
  report.
- **A stateful window.** The window is worked out from the text and the
  cursor alone, so moving the cursor left inside a long value scrolls it as
  soon as the cursor leaves the right edge's run. A field that keeps an
  offset (minimal motion, as `scroll::follow_offset` does for lists) would
  take it as one more input.
- **Selection, undo, and multi-line text.** `tui-input` keeps no selection
  or undo history, and a field is one line. A multi-line editor is a
  separate control (the maintained `ratatui-textarea` continuation), not an
  option of this one.
- **Completion and validity.** A path field's completion, candidate list,
  and validity mark build on `LineEdit` and `TextField` as `PathField`; a
  prompt's title, footer, and error line as `TextPrompt`.
