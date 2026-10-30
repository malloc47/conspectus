# `Checklist<M>`

| | |
| --- | --- |
| Module | `tuikit::checklist` |
| Layer | 1 (control) |
| Type parameters | `M`, the host's message, which the host's `on_confirm` builds from the checked indices; the checklist implements `Overlay<M, Cx>` for every context `Cx` and reads none |
| Cargo feature | none |
| Outcomes | whatever `on_confirm` answers for `Enter` (a host commits its message with `Commit(M)`, or closes with `Close` when nothing is checked); `Close` for `Esc`; `Dismiss` for `q` and `Ctrl-C`; `Consumed` for a move, a check, select all or none, and every other key |

## Purpose

`Checklist<M>` asks which of several items to act on, in a small modal
list any number of whose items the user checks before confirming. Each
item is data (`Item`): a label, an optional detail after it, whether it
is checked, and whether it can change. The bottom border counts the
checked items (`2 of 5 selected`).

It replaces what grows list by list: a checklist that keeps its keys in
a table but never shows them, one that can only be checked an item at a
time, one that says nothing about how many are checked, one with no way
to show an item that cannot change, and one that cuts a long label at
the edge. A checklist written over a third-party list also kept a copy
of its state for every frame and a hand-written `Clone`; this one keeps
plain data and an `Arc` of its host's closure, so it derives `Clone`.

The host keeps which items a checklist offers and in what order, their
labels and details, which start checked and which cannot change, and
what a confirm means: `on_confirm` gets the checked indices, in order,
and maps them back to the values the host listed.

## Usage example

A host that adds toppings asks which ones, with one already checked; it
commits the indices it was given.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tuikit::checklist::{Checklist, Item};
use tuikit::overlay::{Outcome, Overlay};
use tuikit::testkit::buffer::rows;
use tuikit::theme::Theme;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Add(Vec<usize>),
}

let mut list = Checklist::new(
    "Toppings",
    vec![
        Item::new("cheese").checked(true),
        Item::new("onion"),
        Item::new("pepper"),
    ],
    |indices: Vec<usize>| Outcome::Commit(Msg::Add(indices)),
);
let press = |code| KeyEvent::new(code, KeyModifiers::NONE);

// Down, then Space checks the second item.
list.handle(press(KeyCode::Down), &());
list.handle(press(KeyCode::Char(' ')), &());
assert_eq!(list.selected(), [0, 1]);
assert_eq!(list.summary(), "2 of 3 selected");

// `a` checks everything; Enter hands the indices to the host.
list.handle(press(KeyCode::Char('a')), &());
let done = list.handle(press(KeyCode::Enter), &());
assert_eq!(done, Outcome::Commit(Msg::Add(vec![0, 1, 2])));

let theme = Theme::default();
let mut buf = Buffer::empty(Rect::new(0, 0, 60, 14));
list.draw(buf.area, &mut buf, &theme);
let drawn = rows(&buf).join("\n");
assert!(drawn.contains("► √ onion"), "{drawn}");
assert!(drawn.contains("└ 3 of 3 selected "), "{drawn}");
```

The same example is the doctest on the `checklist` module, so it
compiles. The doctest on `Checklist` shows a host that maps the indices
back to its own values, disables an item with a reason, and closes when
nothing is checked.

### A step of another overlay

A checklist that edits one value of an overlay that stays open (a
settings panel's editor for a filter, see
[settings_panel.md](settings_panel.md)) opens as a step of that overlay
(`ModalStack::push_step`), and its `on_confirm` answers `Outcome::Back`:
the stack closes the checklist only, the host applies the message, and
the overlay under it has the keys again. `Esc` steps back to it without
applying, and `Ctrl-C` or `q` dismisses both. A host that holds the
checklist as one level of an overlay of its own instead routes keys to it,
draws it over its own frame with `Checklist::draw` (which keeps the scroll
window), and takes `Close` as stepping back to its own level and
`Dismiss` as dismissing the whole overlay.

### Rows

Inside the frame, from the top: a blank row; one row per item; the
footer under its blank row. An item's row is the cursor gutter, its check mark, a
space, its label, and, when it has one, the separator mark and its
detail. The row is cut to the frame with `…` in cells
(`text::fit_spans`), so the detail is cut before the label is.

- **Marks.** A checked item shows `marks::OK` (`√`) and an unchecked one
  `marks::BULLET` (`•`), the glyph table's marks for a checked and an
  unchecked item in a list of choices. A checked item's mark and label are
  in the theme's `accent`, bold.
- **The count.** The bottom border carries `n of m selected` in the
  frame's footer slot (in the border's color), so the count is in view at
  every size and whatever is scrolled, without a row of its own. Items
  that cannot change count when they are checked.
- **The footer** is generated from `CHECKLIST_KEYS`: `↑/↓ move · Space
  toggle · a select all · n select none · Enter confirm · Esc cancel`,
  drawn by the frame's `ModalFooter` ([frame.md](frame.md#the-footer)):
  receding in `muted`, under a blank row, broken only between hints.

### Size and scrolling

The checklist is the compact modal frame (`CHECKLIST_SIZE`, which is
`ModalSize::COMPACT`: up to 60 columns, at least 20, two columns and a
row clear on each side while the screen allows), as tall as its rows.

When the screen is too short, the blank row above the items goes first.
Then the footer keeps the frame's last rows by the one fit every framed
control shares: all but one, so the cursor's row always shows, its blank
row giving way first and then its first rows, so the keys that leave
stay. The items scroll in a `ScrollWindow` the checklist keeps between
draws, moving no further than the cursor needs. Items that overflow give
their last column to the shared scrollbar (`ScrollGutter`) and are laid
out again one cell narrower, and so is the footer. `Checklist::window` is the window of the last draw; `PgUp` and
`PgDn` move the cursor by its rows.

### Items that cannot change

An item made with `Item::disabled` is drawn receding
(`keys::unavailable_style`: `inert` with `muted`), its detail plain after
it to say why. It keeps the check it was given: the cursor steps over it
in both directions and lands past it after a page or an end key, and
neither `Space` nor select all or none changes it. A checklist in which
nothing can change marks no row, and `Enter` still hands the checked
indices to the host.

## Key table

`CHECKLIST_KEYS`, over `ChecklistKey` (a standard table; see
[keys.md](keys.md)).

| Key | Action | Notes |
| --- | --- | --- |
| `Ctrl-C` | Abort | dismisses the checklist (`Dismiss`), saying nothing; an alias, not in the footer |
| `q` | Abort | as `Ctrl-C`; nothing is typed in a checklist |
| `↑`, `k`, `Ctrl-P` | Up | the previous item that can change, wrapping from the first to the last |
| `↓`, `j`, `Ctrl-N` | Down | the next item that can change, wrapping from the last to the first |
| `PgUp` | PageUp | up by the rows shown, stopping at the first item; an alias |
| `PgDn` | PageDown | down by the rows shown, stopping at the last item; an alias |
| `g`, `Home` | Top | the first item that can change; an alias |
| `G`, `End` | Bottom | the last item that can change; an alias |
| `Space` | Toggle | checks or unchecks the cursor's item |
| `a` | SelectAll | checks every item that can change |
| `n` | SelectNone | unchecks every item that can change |
| `Enter` | Confirm | hands the checked indices to `on_confirm` and reports its answer |
| `Esc` | Cancel | closes the checklist (`Close`), saying nothing; a level of another overlay steps back to it |
| any other key | (none) | swallowed |

Letters bind as typed: `Ctrl-A` is not `a`, and `Ctrl-N` moves down as
in every list.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | the frame's border, its title, and the count on its bottom border; a checked item's mark and label (with bold) |
| `surface` | the background the frame paints |
| `selection_active` | the cursor's row, gutter to the end of its text (`selection::row`) |
| `inert`, `muted` | an item that cannot change: its mark and label (`keys::unavailable_style`) |
| `muted` | the footer (`ModalFooter`) |

Unchecked labels and details are unstyled. The scrollbar is
the shared `ScrollGutter` look.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | each row goes through `selection::row`: the pointer in the 2-cell gutter and the theme's selection pair over the gutter, mark, and label |
| 2 | List ends | a checklist is a short choice list: the arrows wrap (`Ends::Wrap`), stepping over items that cannot change |
| 4 | Close keys | `Esc` closes the checklist's one level; `Ctrl-C` and `q` dismiss it; nothing is typed in a checklist |
| 5 | Close feedback | closing returns `Close` or `Dismiss` and carries no message; the checks are not reported |
| 8 | Footers and hints | the footer is generated from `CHECKLIST_KEYS` in the one grammar and drawn by `ModalFooter` in the one look and fit: receding, under a blank row, keeping its last rows (the keys that leave) on a short screen |
| 10 | Unfocused cursor | an open checklist has the keys, so its cursor row takes `selection_active` |
| 12 | Paging | `PgUp`/`PgDn` move by the rows shown and `Home`/`End`/`g`/`G` go to the ends, stopping there; the window is clamped |
| 13 | Theme | the frame, the checked items, the highlight, and items that cannot change take theme tokens; no raw color or reversed cell |
| 14 | Width | rows are cut with `…` in cells, the detail before the label; the footer breaks between hints |

Not applicable: 3 (the confirm is the list itself; a host that has more to
say asks a dialog after it), 6 and 7 (no fields; the host's `on_confirm`
decides what an empty selection means), 9 (the focus cue is the pane
frame's), 11 (the checklist reports no feedback), 15 and 16 (no text
field), 17 (labels and details are the host's copy), and 18 (pagers).

## Seams left open

- **A range.** Select all and none act on the whole list; checking a run
  of items (`Shift` with the arrows, or a mark and a jump) would join the
  table as another `ChecklistKey` and need a look for the run's anchor.
- **Filtering.** A long list cannot be narrowed by typing; a checklist
  with a query line would share the palette's list model.
- **The count's words.** The bottom border always reads `n of m
  selected`; a host could pass its own words (`3 of 12 to remove`).
- **Groups.** No headings between items; a long list that needs them
  would take a non-selectable row the cursor steps over, as it steps over
  an item that cannot change.
- **Availability from a key table.** An item's `enabled` is set per item;
  the footer's segments could recede when nothing can change.
- **Mouse hit testing.** The frame's rect is the compact size's for the
  rows measured, and each item is one row of the window, which a click
  would be tested against.
