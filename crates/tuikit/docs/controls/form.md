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

- **Override rows.** `Field::with_override(OverrideRow)` makes a text,
  path, or choice field follow an inherited value until the user gives it
  one of their own; the reset key the host's table binds puts the
  inherited value back. Its row ends in a source column that says where
  the value stands. See [Override rows](#override-rows).
- **Explanations.** `Form::intro` puts a paragraph above the rows, and
  `Form::details(n)` reserves `n` lines under them for the focused
  field's hints (`Field::hint`, after an override row's hint); both stay
  in view while the rows scroll, and the band keeps its height as the
  focus moves.
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
`source_cells`, `detail_line`, `paragraph_lines`, `footer_lines`,
`error_line`, `label`, `Cols`) and the body (`FormBody`) are public, for
a host form that still lays out its own rows.

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

### Keeping fields in step

A field whose value follows others (a line derived from them, the items of
a check group that depend on a choice) is the host's to update. The host
keeps the form inside a type of its own, hands it each key, reads
`Form::edited` to see which field the key changed, and updates the
follower through `Form::get_mut`; a change made that way does not clear the
error line. Work that costs something, such as reading a file to decide
what a derived line says, happens there, when an input changes, and its
answer is kept, so drawing the form reads only its fields.

```rust
use ratatui::crossterm::event::KeyEvent;
use tuikit::form::{Field, FieldKind, Form, Tone};
use tuikit::overlay::{Outcome, Overlay};

/// A form whose `total` line follows its `count` field.
struct Order<M> {
    form: Form<M>,
}

impl<M> Order<M> {
    fn handle_key(&mut self, key: KeyEvent) -> Outcome<M> {
        let outcome = Overlay::<M, ()>::handle(&mut self.form, key, &());
        if self.form.edited() == Some("count") {
            let count: u32 = self.form.values().text("count").trim().parse().unwrap_or(0);
            let total = format!("{} cents", count * 250);
            if let Some(FieldKind::ReadOnly(line)) = self.form.get_mut("total").map(Field::kind_mut) {
                line.set(total, Tone::Derived);
            }
        }
        outcome
    }
}
```

### Override rows

Some forms edit a record whose fields have values of their own only where
the user set one; elsewhere a field follows a value the record inherits
(a default, a parent's setting, what was there last time). Such a field is
an override row: `Field::with_override(OverrideRow::new(inherited,
source))` decorates a text, path, or choice field (the field's text is the
value the form opens with, so decorate it once its value is set), and
`OverrideRow::overridden(true)` says the field holds a value of its own as
the form opens. On a toggle, a check group, a one-of-N row, or a
read-only line the decorator has no effect.

- **Editing makes an override.** Typing, stepping a choice, or completing
  a path changes the value, and a submit keeps it as the field's own.
- **Reset follows again.** The reset key (`FormKey::Inherit`, which the
  host binds in its form's key table, such as `Ctrl-R`) puts the
  inherited value back. On a field that held a value of its own, a submit
  then drops that value; on one that did not, the field just follows
  again and nothing is pending. Typing after a reset makes the value the
  field's own again. The key does nothing on a field that is not an
  override row, and a form whose table does not bind it has no reset at
  all: the binding is the form's, never global, so another overlay can
  give the same chord another meaning.
- **The row says where its value stands.** Every text, path, and choice
  row of a form with override rows ends in one source column, right
  aligned and as wide as the longest word any of its rows may show (blank
  on a row that is not an override row), so the words sit in one column
  and the values all end in one place. The word is:

  | `Standing` | When | Word | Look |
  | --- | --- | --- | --- |
  | `Follows` | it follows the inherited value | the source word (`default`) | receding (`muted`) |
  | `Kept` | it holds the value of its own it opened with | `kept` (`KEPT`) | receding (`muted`) |
  | `Changed` | its value changed since the form opened | `will keep` (`WILL_KEEP`) | `warning` |
  | `Reset` | it was reset and held a value of its own | `will follow` (`WILL_FOLLOW`) | `warning` |

- **A hint explains it.** `OverrideRow::hint` takes a closure that gets
  an `OverrideView` (the standing, the value, the inherited value, and the
  source word) and gives the lines the form's detail band shows while the
  row has focus, before the field's own `Field::hint` lines.
- **The host decides what a submit does.** `on_submit` reads each row's
  `Values::standing`: it keeps a `Changed` value, drops a `Reset` one, and
  leaves `Follows` and `Kept` rows alone.

```rust
use ratatui::crossterm::event::KeyCode;
use tuikit::form::{Field, Form, OverrideRow, Standing, Submit};
use tuikit::keys::{FORM_KEYS, FormKey, KeyRow, ctrl, include, key, shown};

#[derive(Debug, PartialEq)]
enum Msg {
    Save { color: Option<String> },
}

/// The form binds its own reset key; no other overlay gets it from here.
const KEYS: &[KeyRow<FormKey>] = &[
    include(FORM_KEYS),
    shown(ctrl('r'), FormKey::Inherit, "Ctrl-R", "reset"),
    shown(key(KeyCode::Enter), FormKey::Submit, "Enter", "save"),
    shown(key(KeyCode::Esc), FormKey::Cancel, "Esc", "cancel"),
];

fn settings(color: &str, own: bool) -> Form<Msg> {
    Form::new("Settings", KEYS, |values| match values.standing("color") {
        Some(Standing::Changed) => Submit::Commit(Msg::Save {
            color: Some(values.text("color").trim().to_string()),
        }),
        Some(Standing::Reset) => Submit::Commit(Msg::Save { color: None }),
        _ => Submit::Close,
    })
    .intro("A field follows the theme until you change it.")
    .details(1)
    .field(
        Field::text("color", "color", color).with_override(
            OverrideRow::new("blue", "theme")
                .overridden(own)
                .hint(|view| vec![format!("the theme says {}", view.inherited)]),
        ),
    )
}
```

The override form of the framework's tests at 64 by 24, with a changed
row and a reset one (`src/form/goldens/override.golden`):

```text
┌ Settings ────────────────────────────────────────────────┐
│  A field follows its default until you change it.        │
│                                                          │
│  title          draft                                    │
│  color          blueish                         will keep│
│► size           medium                        will follow│
│  place          ~/projects/alpha            √     default│
│  tool           saw              [saw] …          default│
│  lock           [ ] locked                               │
│                                                          │
│  the theme's size is medium                              │
│                                                          │
│                                                          │
│↑/↓ move · Tab complete/next · S-Tab back                 │
│←/→/Space change · Ctrl-R reset · Enter save · Esc cancel │
└──────────────────────────────────────────────────────────┘
```

### Size

The form is drawn in the form modal frame (`FORM_SIZE`, which is
`ModalSize::FORM`: up to 80 columns, at least 44, two columns and one row
clear on each side while the screen allows) and is as tall as its intro
and a blank line (when it has one), its rows, a blank line, its detail
lines and a blank line (when it reserves some), the error line while
there is one, the footer, and the border. `Form::rect` is the rect it
draws in. When the frame is shorter, the rows go first: the focused row,
the error line, the footer (its first lines), the lines under the rows
(the blank line, then the detail lines), the intro, and then the other
rows are kept in that order, so a refusal and the row it is about always
show.

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
| the host's reset key (`Ctrl-R` in the tables here) | Inherit | not in `FORM_KEYS`: a form with override rows binds it in its own table. Puts the focused override row's inherited value back; does nothing on another field, and never reaches a field's text |
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
| `muted` | the footer, a choice's other chips, read-only lines (with italics), the intro, the detail lines, and an override row's settled word (its source word or `kept`) |
| `inert` | a path field's `?` mark (nothing typed) |
| `success`, `warning` | a path field's marks; `warning` also for `[custom]`, a `Warning`-tone line, and an override row's pending word (`will keep`, `will follow`) |
| `error` | the error line |

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | the focused row draws the pointer in the 2-cell gutter and the `selection_active` highlight, as a list's cursor row does |
| 2 | List ends | the ring wraps at both ends, as a short choice list does |
| 4 | Close keys | `Esc` closes the form's one level, `Ctrl-C` dismisses it, and `q` is typed (a form is a typing level, option rows included) |
| 5 | Close feedback | closing returns `Close` with no message |
| 6 | Forms | `Enter` submits from any field; `Tab`/`↓` and `S-Tab`/`↑` move over one ring, one stop per field |
| 7 | Validation errors | a refused submit sets one error line in the footer slot and moves the focus to the field it is about; the next edit (a reset included) clears it; nothing is refused silently |
| 8 | Footers and hints | the footer is generated from the form's key table, in the one grammar, and stays in view; a reset key is a row of the form's own table, so its footer names it and no other overlay inherits it |
| 13 | Theme | every row takes theme tokens; no raw color, dim, or reversed cell |
| 14 | Width | labels, values, chips, option rows, source words, the intro, the detail lines, the error line, and the footer are measured and cut in cells |
| 15 | Text editing | text, path, and custom choice fields are `LineEdit`s with the shared keys and paste rule |
| 16 | Path fields | path fields are `PathField`s; the form commits nothing itself, and a host reads `expanded_value()` |
| 17 | Labels | one label column for every row; the labels are the host's, and the host's forms use the short forms |

Not applicable: 3 (a form's submit is its one action; a consequence that
needs a second confirmation is the host's), 9 (the focus cue is the pane
frame's), 10 (a form always has focus while open), 11 (the form reports no
feedback; its refusal is the error line), 12 (a form does not page), and 18
(pagers).

## Seams left open

- **The reset key's availability.** The footer names the reset key on
  every field; a footer that drew it unavailable (`with_availability`)
  while the focused field is not an override row would need the footer
  built per focus instead of once per table.
- **Override rows of other kinds.** A toggle, a check group, or a one-of-N
  row cannot follow a value yet; it would need an inherited state of its
  own kind rather than text.
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
