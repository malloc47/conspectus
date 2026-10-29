# `Form<M>`

| | |
| --- | --- |
| Module | `tuikit::form` |
| Layer | 1 (control) |
| Type parameters | `M`, the host's message, which a submitted form becomes; the form implements `Overlay<M, Cx>` for every context `Cx` and reads none |
| Cargo feature | none |
| Outcomes | `Commit(M)` when `on_submit` takes the values; `Close` for `Esc`, `Ctrl-C`, or an `on_submit` that answers `Submit::Close`; `Consumed` for an edit, a move, a refused submit, or a key that changes nothing; never `Stay(M)` |

## Purpose

`Form<M>` asks for several values in one modal: a name and a directory, a
command and its options, the fields of a record. It is an ordered list of
`Field`s on one ring, submitted from any of them, with one error line and a
footer that stays in view. Every multi-field form a program has uses it, so
each moves, submits, refuses, and shows its keys the same way.

It replaces what forms grow into one at a time: one form whose `Enter`
moves to the next field and another whose `Enter` submits; a row ring for
the arrows and a second ring for `Tab`; a refusal that says nothing, a
second one that posts a toast, and a third one that writes a line the
footer then pushes off the screen; a footer that scrolls away on a short
terminal in one form and stays in the next; a radio row and a checkbox row
drawn with the same `[x]`; and each form's own label column.

- **Fields.** Each `Field` has an id (what the host reads it back by), a
  label, and a `FieldKind`:

  | Kind | Holds | Option keys (`←`, `→`, `Space`) | Typing |
  | --- | --- | --- | --- |
  | `Text(LineEdit)` | one line of text | edit the text (move, type a space) | edits |
  | `Path(PathField)` | a path, with its mark, `Tab` completion, and hint | edit the text | edits |
  | `Choice(Choice)` | one of several named options, shown as chips after the value | step through the options, wrapping | edits when the choice allows a custom value, which its row marks `[custom]` |
  | `Toggle(Toggle)` | one checkbox, whose text may change with its state | flip it | nothing |
  | `CheckGroup(CheckGroup)` | a row of checkboxes | one item: flip it; several: `←`/`→` move the group's cursor, `Space` flips the item under it | nothing |
  | `OneOf(OneOf)` | a row of options, one chosen, drawn `(•)`/`( )` | step the choice, wrapping | nothing |
  | `ReadOnly(ReadOnly)` | a line only read, in a `Tone`: a note, derived output (led in by `└`), or a warning | not a stop | not a stop |

- **Checks.** `Field::validate` checks a text, path, or choice field's
  text (as typed) when the form is submitted. Shown fields are checked top
  to bottom; the first that fails puts its reason on the error line and
  takes the focus. The host's `on_submit` runs only when all pass.
- **Conditions.** `Field::visible_if` shows a field only while a condition
  over the form's `Values` holds (a field that matters only while a toggle
  is on). A hidden field has no row, is not a stop, and is not checked; when
  the focused field is hidden, the focus moves to the stop that took its
  place.
- **One ring.** `Tab`/`↓`/`Ctrl-N` go to the next stop and `S-Tab`/`↑`/
  `Ctrl-P` to the previous, wrapping, one stop per shown field; a read-only
  line is never a stop. `Tab` on a path field completes the path first and
  moves on only when there is nothing to complete.
- **Submit from any field.** `Enter` asks the checks and then `on_submit`
  from whichever field has the focus. `on_submit` answers with a
  `Submit<M>`: `Commit(M)` (the form closes and the host handles `M`),
  `Reject(String)` (the form stays open with the reason on its error line),
  or `Close` (nothing to do, such as a save that changes nothing).
- **One error line,** in the footer slot directly above the footer, in the
  theme's `error`, cut with `…` to the width. The next key that changes a
  value (typing, stepping an option, flipping a toggle, completing a path)
  clears it; moving does not.
- **A fixed footer,** generated from the form's key table and broken only
  between hints, which never scrolls away. When the frame is shorter than
  the form, only the rows scroll, keeping the focused one in view
  (`FormBody`).
- **One grid.** The cursor gutter, the label column (`Cols`: as wide as the
  longest label, at least `LABEL_MIN` (11) and at most `LABEL_MAX` (16)
  cells, a longer label cut with `…`), a two-cell gap, and the value; every
  kind of row starts its value text in the same column.

The host keeps the fields and their order, labels, and seeds; the key
table, with its `Enter` verb; the checks' reasons; and what a submit means.
A host that keeps one field in step with another (a derived line, options
that follow a choice) wraps the form, routes each key to it, and reads
`Form::edited` (the field the last key changed) to update the other through
`Form::get_mut`.

The row kit the form draws with (`text_row`, `path_row`, `choice_row`,
`toggle_row`, `check_row`, `one_of_row`, `static_row`, `read_only_row`,
`footer_lines`, `error_line`, `label`, `Cols`) and the body (`FormBody`)
are public, for a host form that still lays out its own rows.

## Usage example

A host that makes volumes: a required name and one of two sizes.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tuikit::form::{Field, Form, OneOf, Submit};
use tuikit::keys::{FORM_KEYS, FORM_OPTION_KEYS, FormKey, KeyRow, include, key, shown};
use tuikit::overlay::{ModalStack, Outcome};
use tuikit::theme::Theme;

#[derive(Debug, PartialEq)]
enum Msg {
    Create { name: String, size: usize },
}

/// The form's keys: the standard form rows, its own Enter verb, and Esc.
const KEYS: &[KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    include(FORM_OPTION_KEYS),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "create"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

fn volume_form() -> Form<Msg> {
    let sizes = vec!["small".to_string(), "large".to_string()];
    Form::new("New volume", KEYS, |values| {
        Submit::Commit(Msg::Create {
            name: values.text("name").trim().to_string(),
            size: values.chosen("size").unwrap_or(0),
        })
    })
    .field(Field::text("name", "name", "").validate(|text| {
        if text.trim().is_empty() {
            Err("a name is required".into())
        } else {
            Ok(())
        }
    }))
    .field(Field::one_of("size", "size", OneOf::new(sizes, 0)))
}

let mut stack: ModalStack<Form<Msg>> = ModalStack::new();
stack.push(volume_form());
let press = |code| KeyEvent::new(code, KeyModifiers::NONE);

// Enter from the first field: the empty name is refused, and says why.
let outcome = stack.route(press(KeyCode::Enter), &()).expect("a form is open");
assert_eq!(outcome, Outcome::Consumed);
assert_eq!(stack.top().and_then(Form::error), Some("a name is required"));

// Typing clears the reason; the arrows step the size; Enter commits.
stack.route(press(KeyCode::Char('v')), &());
stack.route(press(KeyCode::Down), &());
stack.route(press(KeyCode::Right), &());
let outcome = stack.route(press(KeyCode::Enter), &()).expect("a form is open");
let settled = stack.settle(outcome);
assert_eq!(settled.message, Some(Msg::Create { name: "v".into(), size: 1 }));
assert!(stack.is_empty());

// Drawing: the form frame, the rows, and the fixed footer.
let theme = Theme::default();
let form = volume_form();
let mut buf = Buffer::empty(Rect::new(0, 0, 60, 12));
form.draw(buf.area, &mut buf, &theme);
```

The form drawn at 64 by 20 with an error (the golden in
`src/form/goldens/form.golden` keeps the colors):

```text
┌ Make a thing ────────────────────────────────────────────┐
│  about          a thing to make                          │
│► name           -                                        │
│  place          ~/projects/alpha                        √│
│  copy           [x] keep a copy                          │
│  copies         1                                        │
│  size           (•) small ( ) medium ( ) large           │
│  parts          [ ] lid [x] base                         │
│  tool           saw              [saw] [drill]           │
│                                                          │
│a name is required                                        │
│↑/↓ move · Tab complete/next · S-Tab back                 │
│←/→/Space change · Ctrl-R reset · Enter make · Esc cancel │
└──────────────────────────────────────────────────────────┘
```

and at 40 by 10, where only the focused row, the error line, and the
footer fit (`src/form/goldens/narrow.golden`):

```text
┌ Make a thing ────────────────────────┐
│► name           -                    │
│                                      │
│a name is required                    │
│↑/↓ move · Tab complete/next          │
│S-Tab back · ←/→/Space change         │
│Ctrl-R reset · Enter make · Esc cancel│
└──────────────────────────────────────┘
```

### Size

The form is drawn in the form modal frame (`FORM_SIZE`, which is
`ModalSize::FORM`: up to 80 columns, at least 44, two columns and one row
clear on each side while the screen allows) and is as tall as its rows,
a blank line, the error line while there is one, the footer, and the
border. `Form::rect` is the rect it draws in. When the frame is shorter,
the rows go first: the focused row, the error line, the footer (its first
lines), the blank line, and then the other rows are kept in that order, so a
refusal and the row it is about always show.

## Key table

The host's table, built on `FORM_KEYS` and, for a form with option rows,
`FORM_OPTION_KEYS` (standard tables over `FormKey`; see [keys.md](keys.md)),
then its own `Enter` verb and `Esc cancel`. Keys the table does not bind go
to the focused field ([text_edit.md](text_edit.md) for a text or path
field's editing keys, [path_field.md](path_field.md) for completion).

| Key | Action | Notes |
| --- | --- | --- |
| `Ctrl-C` | Abort | closes the form, saying nothing; an alias, not in the footer |
| `↑`, `Ctrl-P`, `S-Tab` | Up, Back | the previous stop, wrapping |
| `↓`, `Ctrl-N` | Down | the next stop, wrapping |
| `Tab` | Tab | completes the focused path field; the next stop when there is nothing to complete |
| `←`, `→` | OptionPrev, OptionNext | step a choice or a one-of-N row, flip a toggle, move a check group's cursor; on a text or path field, move the text cursor |
| `Space` | Toggle | flip a toggle or the item under a check group's cursor, step a choice or a one-of-N row on; on a text or path field, type a space |
| `Ctrl-R` | Inherit | reaches no field yet (a host table may bind it; the override row is a seam) |
| `Enter` | Submit | checks the shown fields, then asks `on_submit`, from any field |
| `Esc` | Cancel | closes the form, saying nothing |
| any other key | (the focused field) | edits a text or path field, or a choice that allows a custom value; `q` is typed |

The footer of a form with option rows reads `↑/↓ move · Tab complete/next ·
S-Tab back · ←/→/Space change · Enter <verb> · Esc cancel`, wrapped between
hints.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | the frame's border and title, a choice's chosen chip (bold), a text field's scroll marks, a path field's hint |
| `surface` | the background the frame paints |
| `field` | the fill of an unfocused field's value and an option row's text |
| `field_active` | the fill of the focused field's value |
| `field_cursor` | the caret, and the item under a check group's cursor |
| `selection_active` | the highlight of the focused field's row (its fills keep their look) |
| `muted` | the footer, a choice's other chips, and read-only lines (with italics) |
| `inert` | a path field's `?` mark (nothing typed) |
| `success`, `warning` | a path field's marks; `warning` also for `[custom]` and a `Warning`-tone line |
| `error` | the error line |

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | the focused row draws the pointer in the 2-cell gutter and the `selection_active` highlight, as a list's cursor row does |
| 2 | List ends | the ring wraps at both ends, as a short choice list does |
| 4 | Close keys | `Esc` closes the form's one level, `Ctrl-C` dismisses it, and `q` is typed (a form is a typing level, option rows included) |
| 5 | Close feedback | closing returns `Close` with no message |
| 6 | Forms | `Enter` submits from any field; `Tab`/`↓` and `S-Tab`/`↑` move over one ring, one stop per field |
| 7 | Validation errors | a refused submit sets one error line in the footer slot and moves the focus to the field it is about; the next edit clears it; nothing is refused silently |
| 8 | Footers and hints | the footer is generated from the form's key table, in the one grammar, and stays in view |
| 13 | Theme | every row takes theme tokens; no raw color, dim, or reversed cell |
| 14 | Width | labels, values, chips, option rows, the error line, and the footer are measured and cut in cells |
| 15 | Text editing | text, path, and custom choice fields are `LineEdit`s with the shared keys and paste rule |
| 16 | Path fields | path fields are `PathField`s; the form commits nothing itself, and a host reads `expanded_value()` |
| 17 | Labels | one label column for every row; the labels are the host's, and the host's forms use the short forms |

Not applicable: 3 (a form's submit is its one action; a consequence that
needs a second confirmation is the host's), 9 (the focus cue is the pane
frame's), 10 (a form always has focus while open), 11 (the form reports no
feedback; its refusal is the error line), 12 (a form does not page), and 18
(pagers).

## Seams left open

- **An override row.** A field that follows an inherited value until the
  user types one of their own, shows where its value comes from, and goes
  back on `Ctrl-R` (`FormKey::Inherit`, which reaches no field yet).
- **Two controls on one row.** A checkbox beside a text field (a field that
  is revealed by its own toggle) is one stop per control on one line; today
  each is a field of its own.
- **Live checks.** A check runs only on `Enter`; a field that marks a bad
  value while it is typed would take a second closure called on each edit.
- **Field changes that follow other fields.** A host keeps a derived line or
  a choice's options in step by reading `Form::edited` after each key; an
  `on_change` closure on the form would move that into the form.
- **A scrollbar.** When the rows scroll, `FormBody` returns the
  `ScrollWindow` it drew, which a scrollbar gutter would draw from.
- **Mouse hit testing.** `Form::rect` is the rect the form drew in.
