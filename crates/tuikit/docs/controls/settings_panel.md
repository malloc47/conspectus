# `SettingsPanel<Id, M, Cx>`

| | |
| --- | --- |
| Module | `tuikit::settings_panel` |
| Layer | 1 (control) |
| Type parameters | `Id`, the host's identity for a row, which the cursor keeps while the rows are rebuilt; `M`, the host's message, which acting on a row reports; `Cx`, the host's context, from which the host's row model builds the sections on every key and every draw. The panel implements `Overlay<M, Cx>` |
| Cargo feature | none |
| Outcomes | `Stay(M)` when a row's change applies (`Space`, and `Enter` on an editor or a button); `Commit(M)` when `Enter` applies a choice and closes, or a row that leaves the panel is acted on; `Close` for `Esc`, and for `Enter` on a checkbox, a cycle, or the choice in force; `Dismiss` for `q` and `Ctrl-C`; `Consumed` for a move, a row with nothing to do, and every other key |

## Purpose

`SettingsPanel` gathers the settings someone changes while they work (which
view, how it is grouped and sorted, which filters narrow it, how a pane
wraps) in titled sections of one modal, so each is discoverable without a
key of its own. Every change applies as it is made: the panel reports the
row's message at once and stays open to show the new value, and `Esc`
closes it keeping everything, because nothing is pending.

It replaces what such a panel grows by hand: a cursor enum with a variant
per row and a hand-kept list of which rows each context shows, a `match`
per key that sorts rows into choices, editors, and buttons, a line counter
threaded through the drawing so the scroll can find the cursor, a height
budget with magic numbers, a footer drawn in the body that scrolls away
with it, no paging on a list taller than the screen, and editors held as
modes inside the panel with their own check for `Ctrl-C`.

The host keeps which settings there are, in which sections and order, and
which of them a context shows; their labels and the values they show; what
acting on each means (the message a row carries, built from the host's own
state); and its editors: which values a checklist offers, how a prompt
parses its text, and opening them as steps when an editor row's message
arrives.

## Usage example

A host with a few display settings: a checkbox and a choice. `Space`
applies at once and the panel stays open; `Enter` on a choice applies it
and closes.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tuikit::overlay::{Outcome, Overlay};
use tuikit::settings_panel::{Row, Section, SettingsPanel};
use tuikit::testkit::buffer::rows;
use tuikit::theme::Theme;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Wrap(bool),
    Sort(&'static str),
}

struct Prefs {
    wrap: bool,
    sort: &'static str,
}

let mut panel = SettingsPanel::new("Settings", |prefs: &Prefs| {
    let sorts = ["name", "date"].map(|sort| {
        let active = prefs.sort == sort;
        Row::choice(sort, sort, active, (!active).then_some(Msg::Sort(sort)))
    });
    vec![
        Section::new(
            "Display",
            vec![Row::checkbox("wrap", "Wrap long lines", prefs.wrap, Msg::Wrap(!prefs.wrap))],
        ),
        Section::new("Sort", sorts.into()),
    ]
});
let mut prefs = Prefs { wrap: false, sort: "name" };
let press = |code| KeyEvent::new(code, KeyModifiers::NONE);

// Space applies at once and the panel stays open.
let toggled = panel.handle(press(KeyCode::Char(' ')), &prefs);
assert_eq!(toggled, Outcome::Stay(Msg::Wrap(true)));
prefs.wrap = true;

// The cursor steps over the headings; Enter on a choice applies it and
// closes the panel.
panel.handle(press(KeyCode::Down), &prefs);
panel.handle(press(KeyCode::Down), &prefs);
assert_eq!(panel.cursor(), Some(&"date"));
assert_eq!(panel.handle(press(KeyCode::Enter), &prefs), Outcome::Commit(Msg::Sort("date")));

let theme = Theme::default();
let mut buf = Buffer::empty(Rect::new(0, 0, 70, 16));
panel.draw(buf.area, &mut buf, &prefs, &theme);
let drawn = rows(&buf).join("\n");
assert!(drawn.contains("  [x] Wrap long lines"), "{drawn}");
assert!(drawn.contains("  name  active"), "{drawn}");
assert!(drawn.contains("► date"), "{drawn}");
assert!(drawn.contains("Enter apply & close · Esc close"), "{drawn}");
```

The same example is the doctest on the `settings_panel` module, so it
compiles. The doctest on `SettingsPanel` shows an editor row opening a
text prompt as a step and the prompt's answer stepping back to the panel.

### Rows

A row is data (`Row`): the host's `id`, its `kind` with the state it
shows, its `label`, its `action` (the host's message for acting on it, or
`None` when acting on it changes nothing), and whether acting on it
`leaves` the panel. A constructor per kind fills them:

| Kind | Built with | Drawn as | `Space` | `Enter` |
| --- | --- | --- | --- | --- |
| Choice | `Row::choice(id, label, active, select)` | the label, bold with `  active` after it when it is the one in force | reports `select` and stays (`Stay`); nothing when `select` is `None` | reports `select` and closes (`Commit`); closes when `select` is `None` |
| Checkbox | `Row::checkbox(id, label, checked, toggle)` | `[x] label`, bold, or `[ ] label` (`marks::CHECKED`, `marks::UNCHECKED`) | reports `toggle` and stays | closes: `Space` already applied it |
| Editor | `Row::editor(id, name, value, open)` | the name in the name column, the value, and `[ Edit ]` | reports `open` and stays: the host opens the editor as a step | as `Space` |
| Cycle | `Row::cycle(id, name, value, next)` | the name in the name column, the value, and `[ Cycle ]` | reports `next` and stays: the value steps in place | closes: `Space` already applied it |
| Button | `Row::button(id, label, press)` | the label | reports `press` and stays; nothing when `press` is `None` | as `Space` |

`Row::leaves` marks a row whose action hands over to an overlay of its own
(a search, a picker) that takes the panel's place: acting on it reports
`Commit`, which closes the panel. A section (`Section::new(title, rows)`)
is a bold heading over its rows, with a blank line before every section
but the first; the cursor never lands on a heading.

The name column of editor and cycle rows is as wide as the widest of their
names, or as `SettingsPanel::name_width` sets it, so a host whose rows
change with its context keeps the values in one column throughout. Each
line is cut with `…` in cells to fit the frame (`text::fit_spans`), so the
end of a row (its tag, then its value) goes first.

### Changes apply as they are made

The panel holds no copy of a setting. Its row model is a closure over the
host's context, called on every key and every draw, so a row shows the
value in force as soon as the host has applied a change, and the next key
acts on that value. Acting on a row reports the host's message at once:

- `Stay(M)` keeps the panel open while the host applies `M`: a choice
  picked with `Space`, a checkbox toggled, a cycle stepped, an editor
  asked for, a button pressed.
- `Commit(M)` applies and closes: `Enter` on a choice, or any row that
  leaves.
- `Esc` (`Close`), `q`, and `Ctrl-C` (`Dismiss`) close it with nothing to
  report and nothing to undo: every change has been applied already, so
  closing keeps them, and says nothing (house rule 5).

The cursor is the host's `Id`, not a position, so it stays on its row when
the rows around it change (a view switched with `Space` shows other rows
below it). A row the model no longer builds puts the cursor on the first
row.

### Editors are steps

An editor row's message asks the host to open the row's editor: a
`Checklist` (the values a filter keeps) or a `TextPrompt` (a value to type).
The host builds it from its state and opens it as a step over the panel
with `ModalStack::push_step`, and the editor answers with `Outcome::Back`
(a checklist's `on_confirm` returns it; a prompt's returns
`PromptResult::Back`): the stack closes the editor only, the host applies
its message, and the panel is on top again with the new value.

Inside the editor, `Esc` closes it alone and gives the panel its keys back;
`Ctrl-C` (and `q` in a checklist, which types nothing) dismisses the editor
and the panel together, since they are one flow. While the editor is open,
the stack draws the panel with `render_under_step`, which draws its cursor
row in the theme's `selection_inactive`, as a list without focus shows it
(house rule 10). On its own, `SettingsPanel::draw_under_step` does the
same.

### Size and scrolling

The panel is the menu-sized modal frame (`SETTINGS_SIZE`, which is
`ModalSize::MENU`: up to 64 columns, at least 40, two columns and a row
clear on each side while the screen allows), as tall as its sections and
footer: the height comes from the rows, not from a budget.

The footer is drawn by the frame's `ModalFooter`
([frame.md](frame.md#the-footer)): receding in `muted`, under a blank row,
on the frame's last rows. When the screen is too short, it keeps them by
the one fit every framed control shares: all but one, so the cursor's row
always shows, its blank row giving way first and then its first rows, so
the keys that leave stay. The sections scroll in a `ScrollWindow` the
panel keeps between draws, moving no further than the cursor needs. Moving onto the
first row of a section brings its heading into view with it. Lines that
overflow give their last column to the shared scrollbar (`ScrollGutter`)
and are laid out again one cell narrower, and so is the footer.
`SettingsPanel::window` is the
window of the last draw; `PgUp` and `PgDn` move the cursor by its rows,
onto the row nearest the line a page away.

## Key table

`SETTINGS_KEYS`, over `SettingsKey` (a standard table; see
[keys.md](keys.md)).

| Key | Action | Notes |
| --- | --- | --- |
| `Ctrl-C` | Abort | dismisses the panel and any editor open over it (`Dismiss`), saying nothing; an alias, not in the footer |
| `q` | Abort | as `Ctrl-C`; nothing is typed in the panel |
| `↑`, `k`, `Ctrl-P` | Up | the previous row, over the headings, wrapping from the first to the last |
| `↓`, `j`, `Ctrl-N` | Down | the next row, over the headings, wrapping from the last to the first |
| `PgUp` | PageUp | up by the rows shown, stopping at the first row; an alias |
| `PgDn` | PageDown | down by the rows shown, stopping at the last row; an alias |
| `g`, `Home` | Top | the first row; an alias |
| `G`, `End` | Bottom | the last row; an alias |
| `Space` | Change | acts on the cursor's row (see [Rows](#rows)) |
| `Enter` | Apply | applies the cursor's choice and closes, opens an editor, presses a button, or closes on a row `Space` changes |
| `Esc` | Close | closes the panel (`Close`), keeping every change |
| (note) | | `changes apply as you make them`, the first footer segment |
| any other key | (none) | swallowed |

The footer reads `changes apply as you make them · ↑/↓ move · Space
select/toggle · Enter apply & close · Esc close`, broken only between
hints: the note first, where it wraps on a row of its own on a narrow
screen, and the key that leaves last, as every footer lists it.

## Theme tokens

| Token | Used for |
| --- | --- |
| `accent` | the frame's border and its title |
| `surface` | the background the frame paints |
| `selection_active` | the cursor's row, gutter to the end of its text, while the panel has the keys (`selection::row`) |
| `selection_inactive` | the cursor's row while an editor open over the panel has the keys |
| `muted` | the footer (`ModalFooter`) |

Headings, the choice in force, and a checked checkbox are bold; every other
row is unstyled. The scrollbar is the shared `ScrollGutter`
look.

## House rules followed

| # | Rule | How this control follows it |
| --- | --- | --- |
| 1 | List cursor and highlight | each row goes through `selection::row`: the pointer in the 2-cell gutter and the theme's selection pair |
| 2 | List ends | the arrows wrap (`Ends::Wrap`) over the rows, as in every short choice list; headings are never landed on |
| 4 | Close keys | `Esc` closes the panel's one level; `Ctrl-C` and `q` dismiss it, an editor over it included; nothing is typed in the panel, and an editor that types is a prompt of its own |
| 5 | Close feedback | closing returns `Close` or `Dismiss` and carries no message; there is nothing to cancel, since every change applied when it was made |
| 8 | Footers and hints | the footer is generated from `SETTINGS_KEYS` in the one grammar and drawn by `ModalFooter` in the one look and fit: receding, under a blank row, keeping its last rows (the keys that leave) while the sections scroll |
| 10 | Unfocused cursor | under an editor of its own the cursor row takes `selection_inactive` and keeps its marker |
| 12 | Paging | `PgUp`/`PgDn` move by the rows shown and `Home`/`End`/`g`/`G` go to the ends, stopping there; the window is clamped |
| 13 | Theme | the frame and the highlight take theme tokens; emphasis is bold; no raw color or reversed cell |
| 14 | Width | rows and headings are cut with `…` in cells; the footer breaks between hints; the name column is measured in cells |

Not applicable: 3 (no row asks for confirmation; a change is reversible by
acting on the row again, and a host whose row needs a body asks a dialog
from its message), 6 and 7 (no fields; an editor that refuses its input
says why on its own error line), 9 (the focus cue is the pane frame's), 11
(the panel reports no feedback; the host says what a change did), 15 and
16 (no text field; a typed value is a prompt's), 17 (labels are the host's
copy), and 18 (pagers).

## Seams left open

- **Staged apply.** Every change applies when it is made; there is no
  pending state, so `Esc` has nothing to cancel. A panel that staged its
  changes (a dialog of settings with *Apply* and *Cancel*) would keep the
  host's messages in a list instead of reporting them, show the staged
  values over the live ones, commit the list on its apply key, and drop it
  on `Esc`, which would then say `cancel`. Its rows and keys would stay
  the same; only what `Space` reports and what closing means would change.
- **Rows that cannot act now.** Every row the model builds can be landed
  on. A row drawn receding with a reason, stepped over as a menu entry
  that cannot run is, would take an `enabled` field and the
  `keys::unavailable_style` look.
- **Choices cycled in place.** A choice section is one row per option; a
  long list of options could be a cycle row (or a strip of chips on one
  row), which the host can already build from the same messages.
- **The row words.** `active`, `[ Edit ]`, and `[ Cycle ]` are the panel's;
  a host that needs other words (another language) would pass them in.
- **Keys shown per row.** A row could carry an accelerator the host binds
  elsewhere, drawn in a key column as a menu entry's shortcut is; today a
  host writes it into the label (`Notes [1]`), and it acts only outside
  the panel.
- **Action menus under their steps.** The stack tells an entry that a step
  of its own is open over it; the panel recedes its cursor, while an
  action menu keeps its active cursor under its steps for now.
- **Mouse hit testing.** Each row is one line of the window, which a click
  would be tested against once the frame reports its rect.
