# `Dialog<M>`

| | |
| --- | --- |
| Module | `tuikit::dialog` |
| Layer | 1 (control) |
| Type parameters | `M`, the host's message, which a confirmation and each verb of a choice carry (`Clone`, since the dialog keeps them); the dialog implements `Overlay<M, Cx>` for every context `Cx` and reads none |
| Cargo feature | none |
| Outcomes | `Commit(M)` when a confirmation is confirmed (`Enter`, `y`) or a choice's verb is taken (its key); `Close` for `Esc` and `n`, and for `Enter` and `y` in a notice; `Dismiss` for `Ctrl-C` and `q`; `Consumed` for a scroll, `Enter` or `y` in a choice, and every other key; never `Stay(M)` |

## Purpose

`Dialog<M>` asks or tells something in a small modal, in one of three
forms (`Kind<M>`):

- **Confirm** (`Dialog::confirm`): a yes-or-no question before an action
  that a body helps explain, such as one that cannot be undone, with what
  it will affect. `Enter` or `y` commits the host's message; `Esc` or `n`
  closes without it.
- **Choice** (`Dialog::choice`): a question with several named answers,
  each a `Verb<M>` on its own key (`m merge & close`, `d discard &
  close`). Its verb's key commits that verb's message; `Esc` or `n` closes
  without choosing. A choice has no default, so `Enter` and `y` do nothing
  in one.
- **Notice** (`Dialog::notice`): something to read that needs no answer.
  `Enter`, `y`, `Esc`, and `n` close it; it commits nothing.

Each form has a title on its top border, body lines, and an optional
`Warning` list: a heading after the warning mark, in the theme's
`warning`, and the things the action affects, one per row.

It replaces what grows question by question: one confirmation that takes
`y` and `Enter`, a choice with keys and a cancel of its own, and a notice
with a hand-written key hint, each a mode inside the overlay that asked;
three copies of a warning list; screens that lose their keys when the
terminal is short; and a `Ctrl-C` that backs out of the question in one
place and closes everything in another.

The host keeps every word (the title, the body, the warning heading and
its items, each verb), which form a question takes, and the message each
answer commits. It builds the dialog from what it knows when it asks.

## Usage example

A host that deletes, archives, and reports on files asks before it
deletes, offers a choice when a file is closed, and tells the user when
there is nothing to do. The dialog sits on the host's stack like any
overlay.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tuikit::dialog::{Dialog, Verb};
use tuikit::keys::ch;
use tuikit::overlay::{Outcome, Overlay};
use tuikit::testkit::buffer::rows;
use tuikit::theme::Theme;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Delete,
    Archive,
}

let press = |code| KeyEvent::new(code, KeyModifiers::NONE);

// A confirmation: `y` or Enter goes ahead, `n` or Esc does not.
let mut delete = Dialog::confirm("Delete", Msg::Delete)
    .body(["Delete plan.txt?"])
    .warning("open in 2 windows:", ["left", "right"]);
assert_eq!(delete.handle(press(KeyCode::Char('y')), &()), Outcome::Commit(Msg::Delete));
assert_eq!(delete.handle(press(KeyCode::Char('n')), &()), Outcome::Close);

// A choice: its verbs are its keys.
let mut close = Dialog::choice(
    "Close plan.txt",
    vec![
        Verb::new(ch('a'), "archive", Msg::Archive),
        Verb::new(ch('d'), "delete", Msg::Delete),
    ],
)
.body(["Archive it or delete it?"]);
assert_eq!(close.handle(press(KeyCode::Char('a')), &()), Outcome::Commit(Msg::Archive));
assert_eq!(close.handle(press(KeyCode::Enter), &()), Outcome::Consumed, "no default verb");
assert_eq!(close.footer().text(), "a archive · d delete · Esc/n cancel");

// A notice: any of the dismiss keys closes it.
let mut notice = Dialog::<Msg>::notice("Nothing to do").body(["The list is empty."]);
assert_eq!(notice.handle(press(KeyCode::Enter), &()), Outcome::Close);

let theme = Theme::default();
let mut buf = Buffer::empty(Rect::new(0, 0, 60, 14));
delete.draw(buf.area, &mut buf, &theme);
let drawn = rows(&buf).join("\n");
assert!(drawn.contains("▲ open in 2 windows:"), "{drawn}");
assert!(drawn.contains("Enter/y confirm · Esc/n cancel"), "{drawn}");
```

The same example is the doctest on the `dialog` module, so it compiles.

### A question a menu asks

A dialog that a menu's entry leads to opens as a step of the menu. The
entry is marked `Entry::opens_step`, so picking it reports its message
with `Stay` and the menu stays open; the host builds the dialog from the
message and opens it with `ModalStack::push_step`. `Esc` or `n` in the
dialog closes it and the menu has its keys again, with its cursor where
it was; a commit there, or `Ctrl-C` or `q`, closes the dialog and the
menu together. The same dialog opened on its own (`ModalStack::push`) is
an overlay of its own, and `Esc` closes it.

### Rows

Inside the frame, from the top: a blank row; the body's lines, each
wrapped to the frame (an empty line is a blank row); when there is a
warning list, a blank row (if there is a body), the heading after the
warning mark, its continuation rows indented under its first word, and
each item after `  - `, wrapped under its first word; the footer under
its blank row.

- **The footer** is generated from `DIALOG_KEYS`: `Enter/y confirm ·
  Esc/n cancel` for a confirmation; each verb's key and verb, then
  `Esc/n cancel`, for a choice (the cancel segment is the table's,
  through `Footer::only`); and `Enter/Esc close` for a notice, the first
  keys the table shows for confirming and cancelling under the verb
  `close`, since leaving a notice loses nothing. The frame's
  `ModalFooter` draws it ([frame.md](frame.md#the-footer)): receding in
  `muted`, under a blank row, broken only between hints. `Dialog::footer`
  returns it, so a host's conformance case checks the footer the dialog
  draws.
- **A verb's key** is a `KeyMatcher`, so it follows the overlay rule for
  characters (`ch('m')` fires on `m` typed plain or with Shift, never on
  `Ctrl-M`). The dialog resolves its table first, so a verb on a key
  `DIALOG_KEYS` binds (`y`, `n`, `q`, `j`, `k`, `Enter`, `Esc`, the
  arrows, `Ctrl-C`) never fires, and of two verbs on one key only the
  first does. `Dialog::shadowed_verbs` lists such verbs by their key
  labels; a host runs it over the dialogs it builds.

### Size and scrolling

The dialog is the menu-sized modal frame (`DIALOG_SIZE`, which is
`ModalSize::MENU`: up to 64 columns, at least 40, two columns and a row
clear on each side while the screen allows), as tall as its rows. It is as
wide as the menus, so a dialog a menu asks covers the menu where they
overlap and the menu shows above and below it only where the menu is
taller.

When the screen is too short, the blank row above the body goes first.
Then the footer keeps the frame's last rows by the one fit every framed
control shares: every row but one, its blank row giving way first and
then its first rows, which scroll after the body's last row, so the keys
that leave stay. The body scrolls in a `ScrollWindow` the dialog keeps
between draws: the arrows and `k`/`j` scroll it a row, down to any footer
rows that spilled after it. A body that overflows gives its last column to
the shared scrollbar (`ScrollGutter`) and is laid out again one cell
narrower, and so is the footer.
`Dialog::window` is the window of the last draw.

## Key table

`DIALOG_KEYS`, over `DialogKey` (a standard table; see
[keys.md](keys.md)), then a choice's verbs.

| Key | Action | Notes |
| --- | --- | --- |
| `Ctrl-C` | Abort | dismisses the dialog (`Dismiss`) and the flow it is a step of, saying nothing; an alias, not in the footer |
| `q` | Abort | as `Ctrl-C`; nothing is typed in a dialog |
| `↑`, `k` | Up | scrolls a body taller than the dialog up a row; an alias |
| `↓`, `j` | Down | scrolls it down a row; an alias |
| `Enter`, `y` | Confirm | commits a confirmation; closes a notice; does nothing in a choice |
| `Esc`, `n` | Cancel | closes the dialog (`Close`), saying nothing; as a step it gives the overlay under it its keys back |
| a verb's key | (take that verb) | commits the verb's message (a choice) |
| any other key | (none) | swallowed |

The footer of a confirmation reads `Enter/y confirm · Esc/n cancel`.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | the frame's border and its title |
| `surface` | the background the frame paints |
| `warning` | the warning mark and heading (with bold) |
| `muted` | the footer (`ModalFooter`) |

The body and the items are unstyled. The scrollbar is the
shared `ScrollGutter` look.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 3 | Confirmation | the dialog is the idiom when a body helps: `y`/`Enter` confirm, `n`/`Esc` cancel, in every dialog; a choice keeps the same cancel keys beside its verbs |
| 4 | Close keys | `Esc` and `n` close the dialog's one level (as a step, back to what asked); `Ctrl-C` and `q` dismiss it with its flow; nothing is typed in a dialog |
| 5 | Close feedback | closing returns `Close` or `Dismiss` and carries no message |
| 8 | Footers and hints | the footer is generated from `DIALOG_KEYS` (and a choice's verbs) in the one grammar and drawn by `ModalFooter` in the one look and fit: receding, under a blank row, keeping its last rows (the keys that leave) on a short screen |
| 13 | Theme | the frame and the warning heading take theme tokens; no raw color or reversed cell |
| 14 | Width | the body, the heading, and the items wrap in cells, under their first words; the footer breaks between hints |

Not applicable: 1, 2, and 10 (a dialog has no list cursor; a choice's
verbs are keys, not rows), 6 and 7 (no fields, nothing refused), 9 (the
focus cue is the pane frame's), 11 (the dialog reports no feedback), 12
(the body scrolls a row at a time; see the seams), 15 and 16 (no text
field), 17 (every word is the host's), and 18 (pagers).

## Seams left open

- **A default verb.** A choice has no default, so `Enter` and `y` do
  nothing in one. A verb marked as the default would take them and show
  them in its segment (`m/Enter merge & close`).
- **Paging.** A body that scrolls moves a row at a time; a dialog long
  enough to need pages would gain `PgUp`/`PgDn` in `DIALOG_KEYS`, moving
  by `Dialog::window`'s page.
- **Styled body lines.** The body is plain text. A host that needs a
  span of emphasis (a name in bold) would take styled lines, kept to
  theme tokens.
- **Buttons.** The verbs are keys in the footer, not a row of buttons a
  cursor moves over; a pointer-driven host would want them as hit areas.
- **Mouse hit testing.** The dialog's rect is the frame's for the rows it
  measured (`DIALOG_SIZE.rect`), which a click would be tested against.
- **An unfocused menu under a step.** The menu under a dialog keeps its
  active cursor look; a stack that told lower entries they lost focus
  would let it draw `selection_inactive`.
