# `ActionMenu<M>`

| | |
| --- | --- |
| Module | `tuikit::action_menu` |
| Layer | 1 (control) |
| Type parameters | `M`, the host's message, which each entry carries and a pick commits (`Clone`, since the menu keeps its entries); the menu implements `Overlay<M, Cx>` for every context `Cx` and reads none |
| Cargo feature | none |
| Outcomes | `Commit(M)` when an entry is picked (`Enter`, its shortcut, or the second `Enter` on an armed entry); `Stay(M)` instead when the picked entry opens a step; `Close` for `Esc`; `Dismiss` for `q` and `Ctrl-C`; `Consumed` for a move, an arm, a disabled entry's shortcut, and every other key |

## Purpose

`ActionMenu<M>` offers the actions that apply to something (a row, a
selection, the program itself) in a small modal list, and reports the one
the user picks. Each entry is data (`Entry<M>`): a label, an optional
detail line under it, an optional shortcut, whether it can run now, how it
confirms (`Confirm::None` or `Confirm::Inline`), and the message it
commits.

It replaces what grows menu by menu: one menu that stops at its ends and
another that wraps, a menu that cannot scroll and loses its keys on a short
screen beside one that keeps them, a menu with a heading of its own and a
width of its own, a menu whose only way to show a key is to mention it in
prose, and a second `Enter` that one menu asks for without saying it is
waiting for it.

The host keeps which entries a menu offers and in what order, their labels
and details, which of them confirm, and what each commits. It builds the
entries from its own state each time it opens the menu, so the same thing
offers the same actions wherever the menu is opened from.

## Usage example

A host whose files can be opened, shared, and deleted builds the menu for
the file under its cursor and pushes it on its stack. Sharing a file that
is already shared cannot run, and says why; deleting asks twice.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tuikit::action_menu::{ActionMenu, Confirm, Entry};
use tuikit::keys::ch;
use tuikit::overlay::ModalStack;
use tuikit::theme::Theme;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Open(String),
    Share(String),
    Delete(String),
}

struct File {
    name: String,
    shared: bool,
}

/// The actions on `file`: sharing it again cannot run, and deleting it
/// asks twice.
fn file_menu(file: &File) -> ActionMenu<Msg> {
    let mut share = Entry::new("Share", Msg::Share(file.name.clone()));
    share = if file.shared {
        share.detail("already shared").disabled()
    } else {
        share.detail("send a link").shortcut(ch('s'))
    };
    ActionMenu::new(
        file.name.as_str(),
        vec![
            Entry::new("Open", Msg::Open(file.name.clone())).detail("show it"),
            share,
            Entry::new("Delete", Msg::Delete(file.name.clone()))
                .detail("remove it for good")
                .confirm(Confirm::Inline),
        ],
    )
}

let file = File { name: "plan.txt".into(), shared: true };
let mut stack: ModalStack<ActionMenu<Msg>> = ModalStack::new();
stack.push(file_menu(&file));
let press = |code| KeyEvent::new(code, KeyModifiers::NONE);

// Down steps over the entry that cannot run; Delete asks twice.
stack.route(press(KeyCode::Down), &file);
assert_eq!(stack.top().map(ActionMenu::cursor), Some(2));
let first = stack.route(press(KeyCode::Enter), &file).expect("a menu is open");
assert!(stack.settle(first).message.is_none(), "armed, still open");
let second = stack.route(press(KeyCode::Enter), &file).expect("a menu is open");
let settled = stack.settle(second);
assert_eq!(settled.message, Some(Msg::Delete("plan.txt".into())));
assert!(stack.is_empty(), "the pick closed the menu");

// Drawn in the menu-sized frame with its generated footer.
let mut menu = file_menu(&File { name: "plan.txt".into(), shared: false });
let mut buf = Buffer::empty(Rect::new(0, 0, 70, 14));
menu.draw(buf.area, &mut buf, &Theme::default());
assert!(menu.shadowed_shortcuts().is_empty());
```

The same example is the doctest on `ActionMenu`, so it compiles.

### Entries that open a step

An entry whose action needs more than a pick (a dialog with something to
say, a prompt for a value) is marked with `Entry::opens_step`. Picking it
reports its message as `Stay(M)`, so the menu stays open, and the host
builds the question from the message and opens it over the menu with
`ModalStack::push_step`. The menu and its step are one flow: `Esc` in the
step closes it and the menu has the keys again, with its cursor where it
was; a commit in the step, or `Ctrl-C` (and `q` where nothing is typed),
closes the step and the menu together.

```rust
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tuikit::action_menu::{ActionMenu, Entry};
use tuikit::dialog::Dialog;
use tuikit::overlay::{ModalStack, Outcome, Overlay};

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    AskDelete,
    Delete,
}

/// The host's entries: the menu, or the question one of its entries asks.
enum Modal {
    Menu(ActionMenu<Msg>),
    Ask(Dialog<Msg>),
}
// ... `impl Overlay<Msg, ()> for Modal` delegates to each ...

let menu = ActionMenu::new(
    "plan.txt",
    vec![Entry::new("Delete…", Msg::AskDelete).opens_step()],
);
let mut stack = ModalStack::new();
stack.push(Modal::Menu(menu));
// Enter on the entry reports `Stay(Msg::AskDelete)`; the host answers it
// by opening the question as a step of the menu.
stack.push_step(Modal::Ask(
    Dialog::confirm("Delete", Msg::Delete).body(["Delete plan.txt?"]),
));
// Esc in the dialog gives the menu its keys back; `y` would have closed
// both and handed the host `Msg::Delete`.
```

A host that holds a menu as a level inside an overlay of its own routes
keys to it with `Overlay::handle`, takes a `Commit` as the pick, `Close`
as leaving that level, and `Dismiss` as dismissing its whole overlay, and
draws it with `ActionMenu::draw`, which keeps the scroll window as the
stack's draw does.

While a step of the menu's own is open over it, the stack draws the menu
with `render_under_step`, which draws its cursor row in the theme's
`selection_inactive` and keeps its marker, as a list without focus shows
it (house rule 10): the step has the keys. On its own,
`ActionMenu::draw_under_step` does the same.

### Rows

Inside the frame, from the top: a blank row; for each entry its label row
and its detail rows; the footer under its blank row.

- **The label row** starts with the cursor gutter (`selection::row`). When
  any entry has a shortcut, a key column follows: each shortcut's label as
  `keys::key_label` writes it, padded to the widest, and two cells; rows
  without a shortcut leave the column blank, so every label starts in one
  column. A menu with no shortcuts has no key column. A label too long for
  the row is cut with `…`.
- **Detail rows** are the detail wrapped to the width left after an indent
  three cells past the labels, unstyled.
- **The footer** is generated from `MENU_KEYS` (`↑/↓ move · Enter pick ·
  Esc close`) and drawn by the frame's `ModalFooter`
  ([frame.md](frame.md#the-footer)): receding in `muted`, under a blank
  row, broken only between hints.

### Size and scrolling

The menu is the menu-sized modal frame (`MENU_SIZE`, which is
`ModalSize::MENU`: up to 64 columns, at least 40, two columns and a row
clear on each side while the screen allows) and is as tall as its rows
plus the border. One title form, the frame's: no heading inside the body.

When the screen is too short, the footer keeps the frame's last rows by
the one fit every framed control shares: every row but one, its blank row
giving way first and then its first rows, so the keys that leave stay. The
entries scroll in a
`ScrollWindow` the menu keeps between draws: the window moves no further
than it must to show the cursor's entry (its label first, when the entry
is taller than the window), so moving inside the window does not scroll
it. Entries that overflow give their last column to the shared scrollbar
(`ScrollGutter`) and are laid out again one cell narrower, and so is the
footer. The bar's track
keeps a row clear above and below it, so on a screen where the entries
have fewer than five rows (a 40x10 screen gives them four) no bar can
draw and the entries and the footer keep the column
(`ScrollGutter::MIN_HEIGHT`). `ActionMenu::window` is the window of the
last draw.

### Confirming inline

An entry with `Confirm::Inline` arms on the first `Enter` (or its
shortcut) instead of committing. While it is armed its row says what the
next press does, in the theme's `error` and bold: the label, `·`, and
`confirm_note()` (`press Enter again to confirm`, with the key written as
`MENU_KEYS` shows it). The second press commits it. Moving the cursor
disarms it; `Esc`, `q`, and `Ctrl-C` close the menu, armed or not, saying
nothing. An action that needs more than "are you sure" (a list of what it
will affect, a choice of how) belongs in a dialog the entry opens, not in
an inline arm (house rule 3).

### Entries that cannot run

A disabled entry (`Entry::disabled`, `enabled: false`) stays in the list,
so the menu has the same shape whether or not the action can run, and is
drawn receding: its label and key in `keys::unavailable_style` (the
theme's `inert` color with its `muted` modifier). The cursor steps over it
in both directions and is never on it, the cursor starts on the first
entry that can run, and neither `Enter` nor the entry's shortcut commits
it. Its detail stays plain, since it is where the host says why the entry
cannot run. In a menu where nothing can run no row is marked and `Enter`
does nothing; the close keys still close it.

### Shortcuts

`Entry::shortcut` takes a `KeyMatcher`, so a shortcut follows the overlay
rule for characters: `ch('o')` fires on `o` typed plain or with Shift and
never on `Ctrl-O`. A shortcut moves the cursor to its entry and picks it
(arming an inline entry, whose second press commits it). The menu resolves
its table first, so a shortcut on a key `MENU_KEYS` binds (`j`, `k`, `q`,
`Enter`, `Esc`, the arrows, `Ctrl-N`, `Ctrl-P`, `Ctrl-C`) never fires, and
of two entries with one shortcut only the first does.
`ActionMenu::shadowed_shortcuts` lists such shortcuts by their labels; a
host runs it over the menus it builds.

## Key table

`MENU_KEYS`, over `MenuKey` (a standard table; see [keys.md](keys.md)),
then the entries' shortcuts.

| Key | Action | Notes |
| --- | --- | --- |
| `Ctrl-C` | Abort | dismisses the menu (`Dismiss`), saying nothing; an alias, not in the footer |
| `q` | Abort | dismisses the menu, saying nothing; nothing is typed in a menu |
| `↑`, `k`, `Ctrl-P` | Up | the previous entry that can run, wrapping from the first to the last |
| `↓`, `j`, `Ctrl-N` | Down | the next entry that can run, wrapping from the last to the first |
| `Enter` | Pick | commits the cursor's entry (or reports it and stays open, for an entry that opens a step), or arms it when it confirms inline and is not armed |
| `Esc` | Close | closes the menu (`Close`), saying nothing; a menu has one level, and its steps are entries of their own |
| an entry's shortcut | (pick that entry) | moves the cursor to it and picks it; does nothing for an entry that cannot run |
| any other key | (none) | swallowed |

The footer, generated from the table, reads `↑/↓ move · Enter pick · Esc
close`.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | the frame's border and its title |
| `surface` | the background the frame paints |
| `selection_active` | the cursor's label row, gutter to label (`selection::row`) |
| `selection_inactive` | the cursor's label row while a step of the menu's own has the keys |
| `error` | an armed entry's label row (with bold) |
| `inert`, `muted` | a disabled entry's label and key (`keys::unavailable_style`) |
| `muted` | the footer (`ModalFooter`) |

Detail rows are unstyled. The scrollbar is the shared
`ScrollGutter` look.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | each label row goes through `selection::row`: the pointer in the 2-cell gutter and the theme's selection pair over the gutter, key, and label |
| 2 | List ends | a menu is a short choice list: `Ends::Wrap`, stepping over entries that cannot run |
| 3 | Confirmation | `Confirm::Inline` is the inline arm, for an action with nothing more to say; the armed row says what `Enter` will do. A dialog is the idiom when a body helps, opened by an entry that opens a step |
| 4 | Close keys | `Esc` closes the menu's one level; `Ctrl-C` and `q` dismiss it, armed or not, and with it any step over it; nothing is typed in a menu |
| 5 | Close feedback | closing returns `Close` or `Dismiss` and carries no message |
| 8 | Footers and hints | the footer is generated from `MENU_KEYS` in the one grammar and drawn by `ModalFooter` in the one look and fit (receding, under a blank row, keeping its last rows on a short screen); a shortcut is shown in the menu's key column, written as `key_label` writes keys, never in prose |
| 10 | Unfocused cursor | an open menu has the keys, so its cursor row takes `selection_active`; under a step of its own the row takes `selection_inactive` and keeps its marker |
| 13 | Theme | the frame, highlight, armed row, and disabled entries take theme tokens; no raw color or reversed cell |
| 14 | Width | labels are cut with `…`, details wrapped, the key column padded, and the footer broken, all in cells |

Not applicable: 6 and 7 (a menu has no fields and refuses nothing; an
entry that cannot run is shown as such, with its reason in its detail), 9
(the focus cue is the pane frame's), 11 (the menu reports no feedback),
12 (a menu is short; see the seams), 15 and 16 (no text field), 17 (labels
and details are the host's copy), and 18 (pagers).

## Seams left open

- **Paging.** A menu has no `PgUp`/`PgDn` or `Home`/`End`; a menu long
  enough to need them would gain them in `MENU_KEYS` (house rule 12),
  moving by `ActionMenu::window`'s page.
- **Availability from a key table.** `enabled` is set per entry. A host
  whose actions also have key rows could derive it from the rows'
  availability (`keys::with_availability`), so a menu, a footer, and a
  help page dim the same actions from one source.
- **A host's confirm words.** The armed row always says `press Enter again
  to confirm`. An entry could carry its own words (`press Enter again to
  delete`) if a host needs them.
- **Groups.** No separators or headings between entries; a long menu that
  needs them would take a non-selectable row kind the cursor steps over,
  as it steps over a disabled entry.
- **Mouse hit testing.** The menu's rect is the frame's for the rows it
  measured (`MENU_SIZE.rect`), and each entry's rows are known when it is
  laid out, which a click would be tested against.
- **An unfocused menu.** A menu under a step of its own draws its cursor
  inactive; covered by an overlay opened on its own (not a step of it) it
  keeps the active look. A stack that told every lower entry it lost focus
  would let it recede there too.
