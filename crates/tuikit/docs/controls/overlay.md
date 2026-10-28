# `Overlay` and `ModalStack`

| | |
| --- | --- |
| Module | `tuikit::overlay` |
| Layer | 2 (convention) |
| Type parameters | `M`: the host's message, which a commit carries; `Cx`: the host's context, borrowed for every key and every draw; `E`: the stack's entry type, the host's own enum of overlays or a boxed trait object |
| Cargo feature | none |
| Outcomes | `Consumed`, `Close`, `Commit(M)` (close, then the host handles `M`), `Stay(M)` (stay open, and the host handles `M`) |

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
outcome closes the top entry for `Close` and `Commit` and hands the message
back. Every entry is drawn, bottom to top; each clears what it covers, so a
smaller overlay shows the one under it around its frame, and an overlay that
fills the screen hides everything below it.

The module replaces what a program grows one overlay at a time: a key
forwarder and a handler per overlay that differ only in names, a draw call
per overlay in a fixed order unrelated to the stack (so an overlay under
another was never drawn), a context struct copied out of the program's
state before each key to dodge a borrow, and nothing that checked a new
overlay was routed at all. Adding an overlay becomes a state type, an
`Overlay` impl, and one registration in the host's entry type.

The host keeps its entry type and the one place that lists its overlays,
what each message does, any bookkeeping as an overlay closes (settling
hands the closed entry back for it), its base screen (drawn before the
stack unless an entry fills the screen), and each overlay's key table.

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
            Some(MenuKey::Abort | MenuKey::Close) => Outcome::Close,
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
assert!(settled.closed.is_some() && settled.message.is_none());

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
```

### The API

| Item | Does | Notes |
| --- | --- | --- |
| `Overlay::handle(&mut self, key, &cx)` | one key press while the overlay is on top | returns an `Outcome<M>`; performs no effect |
| `Overlay::render(&mut self, area, buf, &cx, &theme)` | draws the overlay over the screen | clears what it covers; `&mut self` so a scrolled overlay keeps the window it drew |
| `Overlay::fills_screen(&self)` | whether the overlay covers the whole screen | `false` by default; nothing under it is drawn |
| `impl Overlay for Box<T>` | a boxed overlay is an overlay | so `ModalStack<Box<dyn Overlay<M, Cx>>>` works |
| `Outcome::{Consumed, Close, Commit(M), Stay(M)}` | what the key asks the host to do | `map`, `closes`, `message` |
| `ModalStack::new`, `push`, `pop`, `pop_if`, `close_all` | open and close entries | `pop_if` closes one kind of overlay only when it is on top; `close_all` returns the entries top first |
| `top`, `top_mut`, `iter`, `iter_mut`, `len`, `is_empty` | look at the entries | `iter` runs bottom to top |
| `route(key, &cx)` | gives the key to the top entry | `None` when the stack is empty: the key is the host's |
| `settle(outcome)` | the stack's part of an outcome | closes the top for `Close` and `Commit`; returns a `Settled { closed, message }` |
| `draw(area, buf, &cx, &theme)` | draws every entry, bottom to top | starts at the topmost entry that fills the screen, on a cleared area |
| `fills_screen()` | whether any open entry fills the screen | the host skips its own screen when it does |

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
| `Esc` | Back one level | `Consumed` while the overlay has a level to return to (a sub-editor, a confirmation, a find prompt, a help page inside a viewer); `Close` at its first level |
| `Ctrl-C` | Dismiss | `Close` from any level; first in every table (`keys::ctrl_c_first`) |
| `q` | Dismiss | `Close` from any level where nothing is typed: a menu, a checklist, a confirmation, a choice, a settings list, a pager, a log, a reader; typed into a prompt, a form, a query, or a find prompt |

The standard tables in [`keys`](keys.md) bind them: `Ctrl-C` first in every
table, and `q` in the pager table.

## Theme tokens

| Token | Used for |
| --- | --- |
| none | the stack draws nothing of its own; it passes the theme to each entry, and clears the area (no style) under an entry that fills the screen |

## House rules followed

| # | Rule | How this module follows it |
| --- | --- | --- |
| 4 | Close keys | `Outcome` names the two ways out: `Consumed` for a step back inside the overlay, `Close` for leaving its first level and for a dismissal from any level; the stack closes only the top entry, so a dismissal never takes the overlay under it too |
| 5 | Close feedback | `Close` carries no message, so a cancel reaches the host as a silent close; a host posts nothing for it |
| 8 | Footers and hints | the stack adds no keys an overlay's table does not have, so every footer generated from a table stays the whole truth |

Not applicable: 1, 2, 10 (list looks and ends are `selection`'s), 3 (a
confirmation is an overlay's own level), 6 (forms), 7 (an error line is the
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
- **Dimming what is underneath.** Every entry draws as it is; a stack that
  dims the entries under the top one would pass each entry whether it is on
  top.
- **Rendered areas.** An entry could report the rect it drew, for mouse hit
  testing and for a conformance check that each entry clears exactly its
  frame.
- **A conformance suite.** The test kit's `assert_conventions` will walk a
  host's registry of overlays and check the close keys above against each.
