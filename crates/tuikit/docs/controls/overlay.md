# `Overlay` and `ModalStack`

| | |
| --- | --- |
| Module | `tuikit::overlay` |
| Layer | 2 (convention) |
| Type parameters | `M`: the host's message, which a commit carries; `Cx`: the host's context, borrowed for every key and every draw; `E`: the stack's entry type, the host's own enum of overlays or a boxed trait object |
| Cargo feature | none |
| Outcomes | `Consumed`, `Close` (step back: close this entry), `Dismiss` (close this entry and every entry it is a step of), `Commit(M)` (close like `Dismiss`, then the host handles `M`), `Back(M)` (close like `Close`, then the host handles `M`), `Stay(M)` (stay open, and the host handles `M`) |

## Purpose

`Overlay<M, Cx>` is the contract every surface that floats over a program's
screen keeps: a prompt, a menu, a dialog, a checklist, a form, a reader, a
palette, a settings panel, or a full-screen viewer. It has two calls.
`handle` takes one key press and the host's context and returns an
`Outcome<M>`; `render` draws the overlay over the screen with the same
context and the theme. An overlay performs no effect; what its message does
is the host's.

`ModalStack<E>` holds the open overlays, bottom to top. The host owns it and
passes its own state as `&Cx` while the stack holds its entries mutably, so
an overlay reads live host data on every key and every frame rather than a
copy taken when it opened. A key goes to the top entry only; settling the
outcome closes the top entry for `Close`, `Back`, `Dismiss`, and `Commit`
and hands the message back. Every entry is drawn, bottom to top; each clears what it
covers, so a smaller overlay shows the one under it around its frame, and an
overlay that fills the screen hides everything below it.

An overlay that asks a further question (a menu whose pick needs a
confirmation, a choice, or a value) opens the question as a *step* of
itself (`push_step`) instead of switching modes inside its own state. The
entry and its steps are one flow: `Esc` in a step is a `Close`, which
closes the step and gives the keys back to the entry under it, and a
`Commit` or a `Dismiss` (`Ctrl-C`, `q`) closes the whole flow, since the
overlay is done or dismissed rather than one level of it. A step that
applies its answer and returns to the entry under it (a settings panel's
editor, whose panel stays open with the new value) answers `Back(M)`:
it closes as `Close` does and hands the host its message. An entry opened
with `push` is an overlay of its own, never part of the flow under it.

While a step is open over an entry, the stack draws that entry with
`render_under_step` instead of `render`. The entry's keys went to a level
of itself, so a list in it can draw its cursor as a list without focus
does (house rule 10). By default the call is `render`, so an overlay that
does not say otherwise looks the same under its steps; an overlay opened
on its own over another is no step of it and leaves it drawn as it is.

The module replaces what a program grows one overlay at a time: a key
forwarder and a handler per overlay that differ only in names, a draw call
per overlay in a fixed order unrelated to the stack (so an overlay under
another was never drawn), a context struct copied out of the program's
state before each key to dodge a borrow, and nothing that checked a new
overlay was routed at all. Adding an overlay becomes a state type, an
`Overlay` impl, and one registration in the host's entry type.

The host keeps its entry type and the one place that lists its overlays,
what each message does, which overlays open as steps of another, any
bookkeeping as an overlay closes (settling hands the closed entries back
for it), its base screen (drawn before the stack unless an entry fills the
screen), and each overlay's key table.

## Usage example

A host with two overlays, one that reads the host's context, on a stack of
the host's own enum. The enum implements the contract by delegating, which
is the one place that lists the overlays.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use tuikit::frame::{Lines, ModalFrame, ModalSize};
use tuikit::keys::{MENU_KEYS, MenuKey, PAGER_KEYS, PagerKey, resolve};
use tuikit::overlay::{ModalStack, Outcome, Overlay};
use tuikit::theme::Theme;

/// The host's messages and the state its overlays read.
#[derive(Debug, PartialEq)]
enum Msg {
    Open(String),
}
struct Model {
    names: Vec<String>,
}

/// A menu over the model's names.
struct Menu {
    cursor: usize,
}

impl Overlay<Msg, Model> for Menu {
    fn handle(&mut self, key: KeyEvent, model: &Model) -> Outcome<Msg> {
        match resolve(MENU_KEYS, key) {
            Some(MenuKey::Abort) => Outcome::Dismiss,
            Some(MenuKey::Close) => Outcome::Close,
            Some(MenuKey::Down) => {
                self.cursor = (self.cursor + 1) % model.names.len();
                Outcome::Consumed
            }
            Some(MenuKey::Pick) => Outcome::Commit(Msg::Open(model.names[self.cursor].clone())),
            Some(MenuKey::Up) | None => Outcome::Consumed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, model: &Model, theme: &Theme) {
        let lines = model.names.iter().map(|name| name.clone().into()).collect();
        let cursor = self.cursor..self.cursor + 1;
        ModalFrame::new(theme, ModalSize::MENU)
            .title("Open")
            .render(area, buf, Lines::new(lines).following(cursor));
    }
}

/// A help page: it never commits, so it works with any message type.
struct Help;

impl<M, Cx> Overlay<M, Cx> for Help {
    fn handle(&mut self, key: KeyEvent, _cx: &Cx) -> Outcome<M> {
        match resolve(PAGER_KEYS, key) {
            Some(PagerKey::Abort | PagerKey::Close) => Outcome::Close,
            _ => Outcome::Consumed,
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, _cx: &Cx, theme: &Theme) {
        ModalFrame::new(theme, ModalSize::WIDE)
            .title("Help")
            .render(area, buf, Lines::new(vec!["Enter opens".into()]));
    }
}

/// The host's entry type: one variant per overlay.
enum Modal {
    Menu(Menu),
    Help(Help),
}

impl Overlay<Msg, Model> for Modal {
    fn handle(&mut self, key: KeyEvent, model: &Model) -> Outcome<Msg> {
        match self {
            Self::Menu(menu) => menu.handle(key, model),
            Self::Help(help) => help.handle(key, model),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, model: &Model, theme: &Theme) {
        match self {
            Self::Menu(menu) => menu.render(area, buf, model, theme),
            Self::Help(help) => help.render(area, buf, model, theme),
        }
    }
}

let model = Model { names: vec!["notes".into(), "plan".into()] };
let mut stack = ModalStack::new();
stack.push(Modal::Menu(Menu { cursor: 0 }));
stack.push(Modal::Help(Help));

let press = |code| KeyEvent::new(code, KeyModifiers::NONE);
// The help page is on top: it takes the key and closes, silently.
let outcome = stack.route(press(KeyCode::Esc), &model).expect("an overlay is open");
let settled = stack.settle(outcome);
assert!(settled.closed.len() == 1 && settled.message.is_none());

// The menu takes the next keys; a pick closes it and hands back a message.
stack.route(press(KeyCode::Down), &model);
let outcome = stack.route(press(KeyCode::Enter), &model).expect("the menu is open");
let settled = stack.settle(outcome);
assert_eq!(settled.message, Some(Msg::Open("plan".into())));
assert!(stack.is_empty());

// Drawing: the host draws its screen, then the stack over it.
let theme = Theme::default();
let area = Rect::new(0, 0, 80, 24);
let mut buf = Buffer::empty(area);
stack.push(Modal::Menu(Menu { cursor: 0 }));
if !stack.fills_screen() {
    // ... the host's own screen ...
}
stack.draw(area, &mut buf, &model, &theme);

// A step: the help page opened as a step of the menu. Esc in it comes
// back to the menu; a dismissal there closes both.
stack.push_step(Modal::Help(Help));
let outcome = stack.route(press(KeyCode::Esc), &model).expect("open");
assert_eq!(stack.settle(outcome).closed.len(), 1);
assert!(matches!(stack.top(), Some(Modal::Menu(_))));
stack.push_step(Modal::Help(Help));
let settled = stack.settle(Outcome::<Msg>::Dismiss);
assert_eq!(settled.closed.len(), 2, "the step and the menu");
assert!(stack.is_empty());
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `Overlay::handle(&mut self, key, &cx)` | one key press while the overlay is on top | returns an `Outcome<M>`; performs no effect |
| `Overlay::render(&mut self, area, buf, &cx, &theme)` | draws the overlay over the screen | clears what it covers; `&mut self` so a scrolled overlay keeps the window it drew |
| `Overlay::render_under_step(&mut self, area, buf, &cx, &theme)` | draws the overlay while a step of its own is open over it | `render` by default; a list draws its cursor with the inactive selection pair |
| `Overlay::fills_screen(&self)` | whether the overlay covers the whole screen | `false` by default; nothing under it is drawn |
| `Overlay::takes_text(&self)` | whether a text field in the overlay has the keys now | `false` by default; a host types a paste only into an overlay that says so |
| `Overlay::hints(&self)` | the keys the overlay takes now, as the footer of the table it resolves them with | empty by default; every framework control returns the footer it draws at its level, so a [status bar](status_bar.md) names the top overlay's keys instead of the screen's |
| `impl Overlay for Box<T>` | a boxed overlay is an overlay | so `ModalStack<Box<dyn Overlay<M, Cx>>>` works |
| `Outcome::{Consumed, Close, Dismiss, Commit(M), Back(M), Stay(M)}` | what the key asks the host to do | `map`, `closes`, `message` |
| `ModalStack::new`, `push`, `push_step`, `pop`, `pop_if`, `close_flow`, `close_all` | open and close entries | `push_step` opens an entry as a step of the one on top (on an empty stack, as `push` does); `pop_if` closes one kind of overlay only when it is on top; `close_flow` closes the top entry and every entry it is a step of; `close_flow` and `close_all` return the entries top first |
| `top`, `top_mut`, `top_is_step`, `iter`, `iter_mut`, `len`, `is_empty` | look at the entries | `iter` runs bottom to top; `top_is_step` says whether a `Close` from the top gives the keys back to the entry under it |
| `route(key, &cx)` | gives the key to the top entry | `None` when the stack is empty: the key is the host's |
| `settle(outcome)` | the stack's part of an outcome | closes the top for `Close` and `Back`, and the top's whole flow for `Commit` and `Dismiss`; returns a `Settled { closed, message }`, `closed` top first |
| `draw(area, buf, &cx, &theme)` | draws every entry, bottom to top | starts at the topmost entry that fills the screen, on a cleared area; an entry with a step of its own over it draws through `render_under_step` |
| `fills_screen()` | whether any open entry fills the screen | the host skips its own screen when it does |
| `takes_text()` | whether the top entry takes a paste | `false` when the stack is empty |

### The host's context

`Cx` is whatever the host's overlays read: usually its whole model,
borrowed. The stack cannot live inside the value it lends as `&Cx` while it
holds its entries mutably, so a host either keeps the stack beside its model
or takes it out of the model for the call and puts it back after. A host
that takes it out lends a model whose stack is empty for that call, so an
overlay reads its own state from `self`, never through the host's stack.

## Key table

The stack binds no keys: an open overlay owns every key, and the stack
gives each one to the top entry. The close keys every overlay's own table
follows (house rule 4), and the outcome each produces:

| Key | Action | Notes |
| --- | --- | --- |
| `Esc` | Back one level | `Consumed` while the overlay has a level of its own to return to (a sub-editor, a find prompt, a help page inside a viewer); `Close` at its first level, which for a step gives the keys back to the entry under it |
| `Ctrl-C` | Dismiss | `Dismiss` from any level, closing every step of the flow; first in every table (`keys::ctrl_c_first`). An overlay that is never a step may answer it with `Close`, which closes it the same way |
| `q` | Dismiss | as `Ctrl-C` wherever nothing is typed: a menu, a checklist, a dialog, a settings list, a pager, a log, a reader; typed into a prompt, a form, a query, or a find prompt |

The standard tables in [`keys`](keys.md) bind them: `Ctrl-C` first in every
table, and `q` in the menu, dialog, checklist, and pager tables.

## Theme tokens

| Token | Used for |
| --- | --- |
| none | the stack draws nothing of its own; it passes the theme to each entry, and clears the area (no style) under an entry that fills the screen |

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 4 | Close keys | `Outcome` names the ways out: `Consumed` for a step back inside the overlay's own state, `Close` for leaving its first level (a step's `Close` returning to the entry under it), `Back` for leaving it with a result, and `Dismiss` for leaving the whole overlay from any level; the stack closes a step's flow for `Dismiss` and `Commit` and never the overlays under the flow |
| 5 | Close feedback | `Close` and `Dismiss` carry no message, so a cancel reaches the host as a silent close; a host posts nothing for it |
| 8 | Footers and hints | the stack adds no keys an overlay's table does not have, so every footer generated from a table stays the whole truth |
| 10 | Unfocused cursor | the stack tells an entry that a step of its own has the keys (`render_under_step`), so its list can show the inactive selection pair |

Not applicable: 1, 2 (list looks and ends are `selection`'s), 3 (a
confirmation is a dialog, opened as a step of what asks), 6 (forms), 7 (an
error line is the
overlay's footer slot), 9 (the focus cue is the pane frame's), 11
(feedback), 12 (paging is each scrolled overlay's), 13 (the stack draws no
style), 14 (width is each overlay's), 15 and 16 (text entry and paths), 17
(labels), 18 (a pager's stepping is the pager's; its `Esc` steps follow
rule 4 through `Consumed`).

## Seams left open

- **A key that is not the overlay's.** An open overlay owns every key, so
  `Outcome` has no "not mine". A variant that lets a key fall through to
  the host (a global key that works over a passive notice) would join
  `Outcome`, and `route` would hand it back.
- **Dimming what is underneath.** An entry under a step of its own is told
  so (`render_under_step`), and the settings panel draws its cursor
  inactive there; an action menu keeps its active cursor under its steps
  for now, and every other entry draws as it is. A stack that dimmed every
  entry under the top one would pass each entry whether it is on top.
- **Rendered areas.** An entry could report the rect it drew, for mouse hit
  testing and for a conformance check that each entry clears exactly its
  frame.
- **A conformance suite.** The test kit's `assert_conventions` walks a
  host's registry of overlays and checks the close keys above against
  each; a step is checked as a case of its own opened over its flow.
