# `keys`

| | |
| --- | --- |
| Module | `tuikit::keys` |
| Layer | 2 (convention) |
| Type parameters | `A`: the action enum of one table (the framework's standard enums, or the host's own) |
| Cargo feature | none |
| Outcomes | none of its own: `resolve` returns the action a key is bound to, or `None` for a key no row binds, and the control turns that into its outcome |

## Purpose

`keys` is the key-table model. Every overlay keeps its keys in a `const`
table of rows over its own action enum, asks `resolve` what a key means,
and matches on the action exhaustively, so a bound key the overlay
ignores fails to compile. Its footer is generated from the same table, so
a footer cannot advertise a key the overlay does not act on, and a help
page can be generated from it too.

The module replaces what a program grows overlay by overlay: hand-written
`match` arms on key codes, footers typed as strings that drift from the
keys, four ways to write a key's name, three footer dialects (`Esc close`,
`Esc to close`, `m = merge`), the same letter matched with and without its
chords in different overlays (so `Ctrl-G` jumped to the top of one pager
and not another), movement rows copied into every pager, and convention
checks that matched action names as strings.

The host keeps the tables of its own overlays (their action enums, verbs,
and notes), what an action does, which actions are available now, and its
program-wide keymap.

## Usage example

A menu on the standard table, and a pager-shaped help page that builds on
the standard pager table with one more close key:

```rust
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tuikit::keys::{
    self, Footer, KeyRow, MENU_KEYS, MenuKey, PAGER_KEYS, PagerKey, ch, include, resolve, shown,
};
use tuikit::theme::Theme;

/// What the host's menu does with a key.
enum Outcome {
    Moved,
    Picked(usize),
    Closed,
    Ignored,
}

struct Menu {
    cursor: usize,
    len: usize,
}

impl Menu {
    fn handle(&mut self, event: KeyEvent) -> Outcome {
        match resolve(MENU_KEYS, event) {
            Some(MenuKey::Abort | MenuKey::Close) => Outcome::Closed,
            Some(MenuKey::Up) => {
                self.cursor = tuikit::selection::Ends::Wrap.step(self.cursor, self.len, -1);
                Outcome::Moved
            }
            Some(MenuKey::Down) => {
                self.cursor = tuikit::selection::Ends::Wrap.step(self.cursor, self.len, 1);
                Outcome::Moved
            }
            Some(MenuKey::Pick) => Outcome::Picked(self.cursor),
            None => Outcome::Ignored,
        }
    }
}

let mut menu = Menu { cursor: 0, len: 3 };
let ctrl_n = KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL);
assert!(matches!(menu.handle(ctrl_n), Outcome::Moved));
assert_eq!(menu.cursor, 1);

// A help page: the standard pager, closed by `?` too.
const HELP_KEYS: &[KeyRow<PagerKey>] = &[
    include(PAGER_KEYS),
    shown(ch('?'), PagerKey::Close, "?", "close"),
];
let theme = Theme::default();
let footer = Footer::new(HELP_KEYS);
let rows = footer.lines(40, &theme);
assert_eq!(rows.len(), 1);

// The checks a host runs over every table it has.
assert!(keys::mislabeled(HELP_KEYS).is_empty());
assert!(keys::unresolved(HELP_KEYS).is_empty());
assert!(keys::loose_characters(HELP_KEYS).is_empty());
assert!(keys::ctrl_c_first(HELP_KEYS));
```

Dimming an action that cannot act now (the copy key of an empty log):
the host copies the table with its availability and draws the copy.

```rust
use tuikit::keys::{Footer, PAGER_KEYS, PagerKey, unavailable_style, with_availability};
use tuikit::theme::Theme;

let theme = Theme::default();
let empty = true;
let rows = with_availability(PAGER_KEYS, |action| !(empty && action == PagerKey::Down));
let footer = Footer::new(&rows);
assert!(footer.segments().iter().all(|segment| segment.available), "j/k still scroll up");
let line = &footer.lines(80, &theme)[0];
assert!(line.spans.iter().all(|span| span.style != unavailable_style(&theme)));
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `KeyMatcher::{Char, Exact, AnyModExceptCtrl, AnyMod}` | which key events a row binds | `matches`, `matches_event`, `example` (the plainest event it fires on) |
| `ch`, `key`, `ctrl`, `alt`, `ctrl_key`, `alt_key` | `const` matcher builders | `key` refuses a character at compile time |
| `key_label` | the one notation for a key | `↑` `↓` `←` `→`, `Space`, `S-Tab`, `PgUp`, `PgDn`, `Ctrl-R`, `Alt-b`, `Shift-Enter` |
| `KeyRow::{Bind, Note, Include}`, `Hint::{Shown, Alias}` | a table's rows | a bound row carries `available` |
| `shown`, `alias`, `include` | `const` row builders | every bound row starts available |
| `resolve` | the action of the first row a key fires | looks inside included tables; ignores availability |
| `rows` | a table's rows with includes spliced in | what footers, help pages, and checks read |
| `with_availability` | a copy of a table with each row's availability set | the seam for dimming, hiding, or explaining an action |
| `Footer`, `Segment`, `footer` | the footer of a table: segments, text, wrapped lines | `Footer::lines` wraps only between segments and dims unavailable ones; `Footer::only` keeps the rows of some actions (a control shows the keys that apply in its state), and a control whose keys are partly its own data (a dialog's verbs) collects its `Segment`s into a `Footer`; `Footer::from(table)` is `Footer::new` |
| `HINT_SEPARATOR` | what joins footer segments | ` · ` |
| `unavailable_style` | how an action that cannot act now is drawn | `inert` color with the `muted` modifier |
| `typed_char` | the character a key types into a text field | plain or shifted, no Control or Alt |
| `KeyRole`, `KeyAction` | what an action is for | `Abort`, `Leave`, `Apply`, `Change`, `Move`, `Page`, `Other` |
| `Convention`, `EnterMeans`, `SpaceMeans` | one modal against the modal key convention | `Convention::breaks` checks it by role |
| `mislabeled`, `unresolved`, `loose_characters`, `ctrl_c_first` | what a table gets wrong | each returns its findings, so a host runs them over every table |

### The rules the model encodes

- **Characters bind as typed.** `ch('g')` fires on `g` alone or with Shift
  (terminals differ on reporting Shift with `G` or `?`), never on `Ctrl-g`
  or `Alt-g`. A chord is spelled out with `ctrl`, `alt`, or `ctrl_key`;
  keys that are not characters (`key`) ignore their modifiers, so a stray
  Shift on an arrow or `Enter` does not stop it. `g` and `G` go to the ends
  of a pager or a log; no chord of them does.
- **Every list moves with `Ctrl-P` and `Ctrl-N`** as well as the arrows: a
  menu, a checklist, a settings list, a picker, a palette, a log, and a
  form's fields. `k` and `j` move too where nothing is typed. A pager has
  no cursor and binds neither chord.
- **`Ctrl-C` comes first** in every table an overlay resolves with, bound
  to an action that dismisses (`KeyRole::Abort`, or `Leave` for one level),
  so no later row can take it. `q` dismisses as well wherever nothing is
  typed: the menu, dialog, checklist, settings, pager, and palette option
  tables bind it; the text, form, and palette tables leave it to type.
- **One footer grammar.** A segment is the keys, a space, and a short
  imperative verb with no "to" and no "=": `Esc close`, `Enter pick`,
  `Tab search current/all`. Labels that share a verb join with `/`; notes
  are short phrases; aliases are not shown.
- **`Esc` says what leaving costs.** `Esc close` where leaving loses
  nothing (a menu, a list, a pager, a settings panel whose changes already
  applied), `Esc cancel` where it drops something pending (a prompt, a
  form, a dialog, a checklist, a find), `Esc back` where it returns to the
  level below in the same overlay. `Enter` names what it does: `pick` in a
  list of choices, `confirm` in a prompt, a dialog, or a checklist, and a
  form's own verb (`save`, `launch`).

## Key table

The standard tables, with the action enums they bind. Aliases are bound but
not shown in the footer.

| Table (enum) | Keys | Footer |
| --- | --- | --- |
| `MENU_KEYS` (`MenuKey`) | `Ctrl-C` and `q` abort; `↑`/`k`/`Ctrl-P` up; `↓`/`j`/`Ctrl-N` down; `Enter` pick; `Esc` close (the table of `ActionMenu`, whose entries' shortcuts come after it) | `↑/↓ move · Enter pick · Esc close` |
| `DIALOG_KEYS` (`DialogKey`) | `Ctrl-C` and `q` abort; `↑`/`k` and `↓`/`j` scroll an overflowing body (aliases); `Enter`/`y` confirm; `Esc`/`n` cancel (the one table of every `Dialog`, whose choice verbs come after it) | `Enter/y confirm · Esc/n cancel` in a confirmation; a choice shows its verbs then `Esc/n cancel` (`Footer::only`), a notice `Enter/Esc close` |
| `PAGER_KEYS` (`PagerKey`) | `Ctrl-C` abort; `j`/`↓` down; `k`/`↑` up; `PgDn`/`Space` page down; `PgUp` page up; `g`/`Home` top; `G`/`End` bottom; `Esc`/`q` close | `j/k/PgDn/PgUp scroll · Esc/q close` |
| `TEXT_INPUT_KEYS` (`TextInputKey`) | `Ctrl-C` abort; `Enter` confirm; `Esc` cancel; `Tab` swallowed; every other key goes to the line editor (the table of `TextPrompt`) | `Enter confirm · Esc cancel` |
| `LINE_EDIT_KEYS` (`EditKey`) | the editing keys every text field shares, all aliases: arrows and `Home`/`End` with their readline chords, word motion on `Alt-b`/`Alt-f` and the Control and Alt arrows, the delete, kill, and yank keys (see [text_edit.md](text_edit.md)) | none |
| `PATH_FIELD_KEYS` (`PathKey`) | `Tab` completes the path, an alias (a path field's own key, consulted after its form's table; see [path_field.md](path_field.md)) | none |
| `CHECKLIST_KEYS` (`ChecklistKey`) | `Ctrl-C` and `q` abort; `↑`/`k`/`Ctrl-P` up; `↓`/`j`/`Ctrl-N` down; `PgUp`/`PgDn` page; `g`/`Home` top and `G`/`End` bottom (aliases); `Space` toggle; `a` select all; `n` select none; `Enter` confirm; `Esc` cancel | `↑/↓ move · Space toggle · a select all · n select none · Enter confirm · Esc cancel` |
| `SETTINGS_KEYS` (`SettingsKey`) | `Ctrl-C` and `q` abort; `↑`/`k`/`Ctrl-P` up; `↓`/`j`/`Ctrl-N` down; `PgUp`/`PgDn` page; `g`/`Home` top and `G`/`End` bottom (aliases); `Space` change; `Enter` apply; `Esc` close; a note that changes apply as they are made (the table of `SettingsPanel`; see [settings_panel.md](settings_panel.md)) | `↑/↓ move · Space select/toggle · Enter apply & close · Esc close · changes apply as you make them` |
| `PALETTE_KEYS` (`PaletteKey`) | `Ctrl-C` abort; `↑`/`Ctrl-P` up; `↓`/`Ctrl-N` down; `PgUp`/`PgDn` page (aliases); `Enter` pick; `Esc` cancel; every other key edits the query, so `q`, `j`, and `k` type (the table of `Palette`; a palette with a scope or options includes it and binds `PaletteKey::Scope` and `PaletteKey::Options` with its own verbs, by convention on `Tab` and `Ctrl-O`; see [palette.md](palette.md)) | `↑/↓ move · Enter pick · Esc close` |
| `PALETTE_OPTION_KEYS` (`PaletteOptionKey`) | `Ctrl-C` and `q` abort; `↑`/`k`/`Ctrl-P` up; `↓`/`j`/`Ctrl-N` down; `Space` and `Enter` toggle; `Esc` and `Ctrl-O` back (a palette's options pane) | `↑/↓ move · Space toggle · Esc back` |
| `FORM_KEYS` (`FormKey`) | `Ctrl-C` abort; `↑`/`Ctrl-P` previous field; `↓`/`Ctrl-N` next field; `Tab` complete or next; `S-Tab` back | `↑/↓ move · Tab complete/next · S-Tab back` |
| `FORM_OPTION_KEYS` (`FormKey`) | `←` option back; `→` option on; `Space` toggle | `←/→/Space change` |

A form's table includes `FORM_KEYS` first, then `FORM_OPTION_KEYS` if it
has option or toggle rows, its own rows, its `Enter` row with the form's
verb (`Enter save`), and `Esc cancel`. `Space` is its own action,
`FormKey::Toggle`, so a row of several checkboxes can move between them
with `←`/`→` and flip one with `Space`; on every other option row it steps
the option on as `→` does (see [form.md](form.md)). `FormKey::Inherit`
drops a field's own value and takes the inherited one again, for forms
with override rows; no standard table binds it, so a form binds its own
reset key (a chord such as `Ctrl-R`) in its own table and the chord stays
free to mean something else in another overlay. `PagerKey::Copy` is bound
by a pager whose content can be copied.

## Theme tokens

| Token | Used for |
| --- | --- |
| `inert` | an unavailable action's footer segment or help row (`unavailable_style`) |
| `muted` | the same, so it recedes even where a footer is drawn in `inert` |

A footer's available segments and its separators are unstyled: they take
the style the control draws its footer in.

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 3 | Confirmation | `DIALOG_KEYS` is the one dialog keymap: `y`/`Enter` confirm, `n`/`Esc` cancel, in a confirmation, a choice (beside its verbs), and a notice alike |
| 4 | Close keys | `Esc` leaves one level in every standard table; `Ctrl-C` comes first and dismisses (`ctrl_c_first`); `q` dismisses too in every table where nothing is typed (menus, dialogs, checklists, settings panels, pagers, a palette's options pane) and types in the text, form, and palette tables |
| 6 | Forms | `FORM_KEYS`: `Tab`/`↓`/`Ctrl-N` next and `S-Tab`/`↑`/`Ctrl-P` previous; `Enter` is the form's submit row |
| 8 | Footers and hints | every footer is generated from its table in one grammar; `unresolved` fails a footer that names a key an earlier row takes |
| 12 | Paging | `PAGER_KEYS`, `CHECKLIST_KEYS`, and `SETTINGS_KEYS` bind `PgUp`/`PgDn`, `Home`/`End`, and `g`/`G` as typed, and `PALETTE_KEYS` binds `PgUp`/`PgDn` (its `Home`/`End` move the query's caret and `g`/`G` type); the control sizes the page |
| 13 | Theme | the one look it adds, for an unavailable action, comes from `inert` and `muted` |
| 14 | Width | `Footer::lines` measures segments in cells and wraps only between them |

Not applicable: 1, 2, and 10 (the list's look and ends are `selection`'s),
5 and 7 (feedback and errors are the control's), 9 (the focus cue is the
pane frame's), 11 (feedback severity), 15 and 16 (editing and paths are
the text-entry controls', though the editing keys' table is kept here with
the others), 17 (labels are the control's copy), and 18 (the
pager's stepping is the pager's; its keys come from here). Half-page
`Ctrl-D`/`Ctrl-U` join `PAGER_KEYS` with the pager control.

## Seams left open

- **Tri-state availability.** A row is available or not, and a footer and
  a help page dim what is not. Hiding an action entirely (the third state)
  is a filter over `with_availability`'s copy; explaining why it is
  unavailable is the control's message when the key still resolves.
- **A scoped program-wide keymap.** The host's main keymap keeps its own
  table for now. A later step adds scopes (program, pane focus, overlay)
  whose rows carry help text, built on `KeyMatcher` and `KeyRow`.
- **Generated help pages.** `rows` and `key_label` are what the `help`
  module's generator reads: `help::sections_for` turns a table and a
  description per action into a help page, and `help::undescribed` finds
  a key it would leave out (see [help.md](help.md)).
- **Key notation round trip.** `key_label` writes keys; parsing the same
  notation back into a matcher (for configuration or a test key DSL) would
  sit beside it.
- **User remapping.** Tables are `const`; a remapped table is a copy with
  other matchers, built the way `with_availability` builds its copy.
