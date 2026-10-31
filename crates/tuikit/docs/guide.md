# Building a program on the framework

This guide walks through what a program does to use the framework: depend on
it, give it the program's own message and context types, draw a screen and
its modals, register the program's colors and glyphs, write its keys, show
help, hints, and feedback, and test it all without a terminal. The
[architecture guide](architecture.md) explains why the pieces are shaped the
way they are, the [reference pages](controls/_template.md) document each
module, and the [house rules](house-rules.md) say how every control behaves.

The examples build a small file browser. Every `rust` block here compiles and
runs as one of the crate's doctests, so the guide cannot drift from the code.

## What the framework does and what the program does

The framework draws and reads keys. A program built on it keeps everything
that gives the screen meaning, and runs everything that acts:

| The framework | The program (the host) |
| --- | --- |
| Design tokens, glyphs, and text measured in cells | Its own colors and glyphs, registered by name |
| Controls that draw and answer a key with an outcome | What each outcome means and does |
| Key tables, footers, hints, and help generated from them | Which actions it has, their keys and words |
| The overlay contract and a modal stack | The stack itself, kept in its state |
| Toasts, a message log, and an OSC 52 encoder | The clock, the log's payload, the terminal's writer |
| A test kit that drives and checks frames | Its own registry of modals and its golden files |

The program also owns the runtime: the event loop, terminal setup and
teardown (raw mode, the alternate screen, bracketed paste), the effects its
messages run, undo, and handing the terminal to a child process. No control
starts a process, writes a file, or reads global state.

## Depend on it

Depend on the crate by path, relative to the program's manifest:

```toml
[dependencies]
tuikit = { path = "../tuikit" }

[dev-dependencies]
tuikit = { path = "../tuikit", features = ["testkit"] }
```

Two features add to the build only what a program asks for:

| Feature | Turns on | Pulls in |
| --- | --- | --- |
| `richtext` | `tuikit::richtext`: ANSI text, a captured terminal screen fitted to an area, Markdown with tables and highlighted code | three parsing crates and a syntax highlighter |
| `testkit` | `tuikit::testkit`: key scripts, the frame driver, buffer probes, goldens, the conformance suite | nothing |

Turn `testkit` on from the dev-dependencies (and from any dev-only feature of
the program's own that draws frames off screen), so production builds leave it
out. The public API exposes ratatui types, so the program uses the ratatui
version the framework's manifest names, with its crossterm backend; key
events are `ratatui::crossterm::event::KeyEvent`.

## The program's types

A control never names a type of the program's. It is generic over what it
needs to say and know instead:

- **`M`, the message.** A control that commits something (a confirmed
  prompt, a picked menu entry, a submitted form) reports an `M` the program
  gave it or built through a closure the program gave it. `M` is usually the
  program's own message enum.
- **`Cx`, the context.** A control that reads the program's state on every
  key and every draw (a settings panel's rows, a log list's log) is handed
  `&Cx`, a borrow of that state. Nothing is copied when the control opens,
  so it always shows the state as it is.
- **`Id` and `R`.** A tree or a settings panel keeps its cursor by the
  program's identity for a row (`Id`), so the cursor survives a rebuild of
  the rows; a palette or a tree stores the program's row type (`R`) and
  hands it back without looking inside.

A control takes only the parameters it uses, and its page lists them in its
header table.

## A program in one page

The file browser below has a list of files, a key that deletes the file under
the cursor after asking, a status bar that names the keys that act now, and a
toast when a file is gone. It is the whole shape of a program on the
framework: one key handler that offers each key to the modal stack first and
then to the program's keymap, one `update` where messages take effect, and
one `Screen` that `AppFrame` draws in layers.

```rust
use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use tuikit::app_frame::{AppFrame, Screen};
use tuikit::dialog::Dialog;
use tuikit::feedback::{Feedback, Toasts};
use tuikit::frame::PaneFrame;
use tuikit::keys::{ActiveScopes, Bind, Footer, Keymap, Scope, bind, ch, ctrl, key};
use tuikit::overlay::{ModalStack, Outcome, Overlay};
use tuikit::selection::{self, Cursor, Ends};
use tuikit::status_bar::{Annotation, KeyHintBar, StatusBar};
use tuikit::testkit::buffer::rows;
use tuikit::testkit::keys::script;
use tuikit::theme::Theme;

/// What the program knows: the context its overlays and its keys read.
struct Files {
    names: Vec<String>,
    cursor: usize,
}

/// What an overlay can ask the program to do.
#[derive(Clone, Debug, PartialEq)]
enum Msg {
    Delete(String),
}

/// The actions the program's own keys stand for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Do {
    Up,
    Down,
    Delete,
    Quit,
}

const LIST: Scope = Scope::Pane("files");

/// The program's keys: one table, which also gives the status bar its
/// hints and a help page its rows.
const KEYS: &[Bind<Do, Files>] = &[
    bind("files.up", LIST, &[key(KeyCode::Up), ch('k')], Do::Up)
        .verb("move")
        .help("Files", "Move to the previous file"),
    bind("files.down", LIST, &[key(KeyCode::Down), ch('j')], Do::Down)
        .verb("move")
        .help("Files", "Move to the next file"),
    bind("files.delete", LIST, &[ch('d')], Do::Delete)
        .verb("delete")
        .help("Files", "Delete the file under the cursor")
        .available_when(|files: &Files| !files.names.is_empty()),
    bind("quit", Scope::Global, &[ch('q'), ctrl('c')], Do::Quit)
        .verb("quit")
        .help("Program", "Quit"),
];

/// The program's overlays: the one place that lists them.
enum Modal {
    Confirm(Dialog<Msg>),
}

impl Overlay<Msg, Files> for Modal {
    fn handle(&mut self, key: KeyEvent, files: &Files) -> Outcome<Msg> {
        match self {
            Self::Confirm(dialog) => dialog.handle(key, files),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, files: &Files, theme: &Theme) {
        match self {
            Self::Confirm(dialog) => dialog.render(area, buf, files, theme),
        }
    }

    fn hints(&self) -> Footer {
        match self {
            // A dialog is an overlay for any context; name this one.
            Self::Confirm(dialog) => Overlay::<Msg, Files>::hints(dialog),
        }
    }
}

struct Program {
    theme: Theme,
    files: Files,
    modals: ModalStack<Modal>,
    keymap: Keymap<Do, Files>,
    toasts: Toasts,
    status: Option<Feedback>,
    running: bool,
}

impl Program {
    fn new(names: &[&str]) -> Self {
        Self {
            theme: Theme::default(),
            files: Files { names: names.iter().map(|name| name.to_string()).collect(), cursor: 0 },
            modals: ModalStack::new(),
            keymap: Keymap::builder().rows(KEYS).build().expect("no key shadows another"),
            toasts: Toasts::new(),
            status: None,
            running: true,
        }
    }

    /// One key: an open overlay owns every key; otherwise the keymap
    /// looks it up in the scopes that have the keys.
    fn press(&mut self, key: KeyEvent) {
        self.status = None;
        if let Some(outcome) = self.modals.route(key, &self.files) {
            if let Some(message) = self.modals.settle(outcome).message {
                self.update(message);
            }
            return;
        }
        let Some(binding) = self.keymap.resolve(ActiveScopes::pane("files"), key) else {
            return;
        };
        let (action, can_act) = (binding.action(), binding.can_act(&self.files));
        if !can_act {
            self.status = Some(Feedback::warning("nothing to delete"));
            return;
        }
        let len = self.files.names.len();
        match action {
            Do::Up => self.files.cursor = Ends::Clamp.step(self.files.cursor, len, -1),
            Do::Down => self.files.cursor = Ends::Clamp.step(self.files.cursor, len, 1),
            Do::Delete => {
                let name = self.files.names[self.files.cursor].clone();
                let ask = Dialog::confirm("Delete", Msg::Delete(name.clone()))
                    .body([format!("Delete {name}?")]);
                self.modals.push(Modal::Confirm(ask));
            }
            Do::Quit => self.running = false,
        }
    }

    /// Where messages take effect: the program's runtime, not a control.
    fn update(&mut self, message: Msg) {
        match message {
            Msg::Delete(name) => {
                self.files.names.retain(|kept| *kept != name);
                self.files.cursor = self.files.cursor.min(self.files.names.len().saturating_sub(1));
                self.toasts.show(Feedback::success(format!("deleted {name}")), Instant::now());
            }
        }
    }
}

impl Screen for Program {
    type Report = ();

    fn header(&mut self, area: Rect, buf: &mut Buffer) {
        Paragraph::new(format!(" {} files", self.files.names.len())).render(area, buf);
    }

    fn body(&mut self, area: Rect, buf: &mut Buffer) {
        let focused = self.modals.is_empty();
        let inner = PaneFrame::new(&self.theme)
            .title(Line::from("files"))
            .focused(focused)
            .render(area, buf);
        let lines: Vec<Line> = (self.files.names.iter().enumerate())
            .map(|(at, name)| {
                let cursor = Cursor::of(at == self.files.cursor, focused);
                selection::row([Span::raw(name.as_str())], cursor, &self.theme)
            })
            .collect();
        Paragraph::new(lines).render(inner, buf);
    }

    fn footer(&mut self, area: Rect, buf: &mut Buffer) {
        // The keys that act now: the top overlay's, or the program's own.
        let hints = match self.modals.top() {
            Some(top) => KeyHintBar::new().footer(&top.hints(), 1),
            None => KeyHintBar::new().keymap(&self.keymap, ActiveScopes::pane("files"), &self.files, 1, |_| {
                Some(Annotation::new())
            }),
        };
        StatusBar::new(&self.theme).message(self.status.as_ref()).hints(&hints).render(area, buf);
    }

    fn overlays_fill_screen(&self) -> bool {
        self.modals.fills_screen()
    }

    fn overlays(&mut self, area: Rect, buf: &mut Buffer) {
        self.modals.draw(area, buf, &self.files, &self.theme);
    }

    fn toasts(&mut self, area: Rect, buf: &mut Buffer) {
        self.toasts.render(area, buf, &self.theme);
    }
}

let mut program = Program::new(&["notes.txt", "plan.txt"]);
let frame = AppFrame::new();
let area = Rect::new(0, 0, 50, 12);
let draw = |program: &mut Program| {
    let mut buf = Buffer::empty(area);
    frame.render(area, &mut buf, program);
    rows(&buf)
};

// The first frame: the files, and the keys that act on them.
let shown = draw(&mut program);
assert!(shown[1].starts_with("┌ ► files"));
assert!(shown[11].starts_with("↑/↓ move · d delete · q quit"));

// `j` moves to the second file and `d` asks first: the dialog has the keys,
// and the status bar names its keys instead of the program's.
for key in script("jd") {
    program.press(key);
}
assert!(draw(&mut program)[11].starts_with("Enter/y confirm · Esc/n cancel"));

// `y` confirms: the stack closes the dialog and hands the program its
// message, and the program deletes the file and says so.
program.press(script("y")[0]);
assert_eq!(program.files.names, ["notes.txt"]);
assert!(draw(&mut program).iter().any(|row| row.contains("deleted plan.txt")));
assert!(program.running);
```

A live program wraps the same `press` and `render` in its own loop: read an
event, hand a key to `press`, draw a frame with `AppFrame::render` inside
`Terminal::draw`, and call `Toasts::tick` with the clock so expired toasts go.
The sections below take the pieces one at a time.

## The screen: `AppFrame` and `Screen`

`AppFrame` draws a screen in layers: the header row, the body, and the footer
row, then the overlays from the bottom of the stack to its top, then the
toasts. The program implements `Screen` (usually on a small adapter over its
own state) to say what each part draws; only `body` is required. While an
overlay fills the screen, the chrome is not drawn at all.

The body returns a `Report`: what it measured, such as the scroll windows of
its panes. `render` hands it back, and the program keeps it for the next key
(a page key moves by the rows the last frame showed). Nothing is sent from
inside a draw, so a frame stays a function of the state and the area.
`AppFrame::areas` gives the same layout to anything that needs one part's
cells again.

A body of two panes splits its area with `frame::SplitPane` (side by side on a
wide screen, stacked on a narrow one) and draws each pane in a
`frame::PaneFrame`, whose border and title marker take the focus accent while
the pane has focus (house rule 9). See [app_frame.md](controls/app_frame.md)
and [frame.md](controls/frame.md).

## The modal stack

Every surface that floats over the screen (a prompt, a menu, a dialog, a
form, a reader, a palette) implements `Overlay<M, Cx>`: `handle` answers one
key with an `Outcome<M>`, and `render` draws it. The program keeps one
`ModalStack<E>` in its state, where `E` is either its own enum of overlays,
as above, or `Box<dyn Overlay<M, Cx>>`. The enum is the one place that lists
the program's overlays; it delegates each call to the overlay inside,
including the optional ones (`render_under_step`, `fills_screen`,
`takes_text`, `hints`) of the overlays that have them.

- **Keys.** `route(key, &cx)` gives a key to the top entry, or returns `None`
  when the stack is empty and the key is the program's. An open overlay owns
  every key, so the program offers each key to the stack first.
- **Outcomes.** `settle(outcome)` does the stack's part: it closes the top
  entry for `Close` and `Back`, the top entry and every entry it is a step of
  for `Commit` and `Dismiss`, and nothing for `Consumed` and `Stay`. It
  returns the entries it closed and the message to handle. Closing posts
  nothing (house rule 5).
- **Steps.** An overlay that asks a further question opens it as a step of
  itself. A menu entry marked `opens_step` reports its message with `Stay`;
  the program builds the question and opens it with `push_step`. `Esc` in the
  step returns to the menu; a commit or `Ctrl-C` closes both.
- **Drawing.** `draw(area, buf, &cx, &theme)` draws every entry, bottom to
  top, starting at the topmost one that fills the screen.
- **The context.** The stack holds its entries mutably while it lends the
  program's state as `&Cx`, so keep the stack beside the state (as `Program`
  keeps `modals` beside `files`), or take it out of the state for the call.
- **Paste.** A program that turns bracketed paste on types a pasted text only
  when `takes_text()` says a text field has the keys, and flattens it to one
  line first, so a pasted line break cannot submit anything.

A menu whose entry asks before it acts:

```rust
use ratatui::crossterm::event::{KeyCode, KeyEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuikit::action_menu::{ActionMenu, Entry};
use tuikit::dialog::Dialog;
use tuikit::overlay::{ModalStack, Outcome, Overlay};
use tuikit::testkit::keys::{press, typed};
use tuikit::text_edit::single_line_paste;
use tuikit::theme::Theme;

#[derive(Clone, Debug, PartialEq)]
enum Msg {
    AskDelete,
    Delete,
}

enum Modal {
    Menu(ActionMenu<Msg>),
    Ask(Dialog<Msg>),
}

impl Overlay<Msg, ()> for Modal {
    fn handle(&mut self, key: KeyEvent, cx: &()) -> Outcome<Msg> {
        match self {
            Self::Menu(menu) => menu.handle(key, cx),
            Self::Ask(dialog) => dialog.handle(key, cx),
        }
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, cx: &(), theme: &Theme) {
        match self {
            Self::Menu(menu) => menu.render(area, buf, cx, theme),
            Self::Ask(dialog) => dialog.render(area, buf, cx, theme),
        }
    }

    // Under its own step a menu draws its cursor inactive.
    fn render_under_step(&mut self, area: Rect, buf: &mut Buffer, cx: &(), theme: &Theme) {
        match self {
            Self::Menu(menu) => menu.render_under_step(area, buf, cx, theme),
            Self::Ask(dialog) => dialog.render_under_step(area, buf, cx, theme),
        }
    }
}

let mut stack = ModalStack::new();
let menu = ActionMenu::new("notes.txt", vec![Entry::new("Delete…", Msg::AskDelete).opens_step()]);
stack.push(Modal::Menu(menu));

// Enter on the entry reports its message and the menu stays open; the
// program answers by opening the question as a step of the menu.
let outcome = stack.route(press(KeyCode::Enter), &()).expect("the menu is open");
assert_eq!(outcome, Outcome::Stay(Msg::AskDelete));
assert_eq!(stack.settle(outcome).message, Some(Msg::AskDelete));
stack.push_step(Modal::Ask(Dialog::confirm("Delete", Msg::Delete).body(["Delete notes.txt?"])));

// Esc in the step goes back to the menu; `y` would close both and hand
// the program `Msg::Delete`.
let outcome = stack.route(press(KeyCode::Esc), &()).expect("the dialog is open");
assert_eq!(stack.settle(outcome).closed.len(), 1);
assert!(matches!(stack.top(), Some(Modal::Menu(_))));

// Nothing here takes text, so a paste is not typed into the menu.
assert!(!stack.takes_text());
let pasted = single_line_paste("two\nlines");
if stack.takes_text() {
    for c in pasted.chars() {
        stack.route(typed(c), &());
    }
}
```

The framework's overlays are listed on their pages: a one-line prompt
([text_prompt.md](controls/text_prompt.md)), a form
([form.md](controls/form.md)), an action menu
([action_menu.md](controls/action_menu.md)), a dialog
([dialog.md](controls/dialog.md)), a checklist
([checklist.md](controls/checklist.md)), a palette
([palette.md](controls/palette.md)), a settings panel
([settings_panel.md](controls/settings_panel.md)), a reader
([pager.md](controls/pager.md)), a help page
([help.md](controls/help.md)), and a log reader
([log_list.md](controls/log_list.md)). The contract itself is
[overlay.md](controls/overlay.md).

## Theme roles and glyphs

`Theme::default()` holds the framework's tokens: the accent, the two muted
looks, the selection pair, the four severities, links, matches, chips, the
recency scale, surfaces, and input fields. Every control takes `&Theme` and
names no color of its own (house rule 13), so changing the theme restyles
every control at once.

A role only the program knows (a color per kind of file, per state of a job)
is a named style the program registers once and reads by name wherever it
draws that role. An older configuration key that should keep working after a
rename is an alias. The program reads its own configuration file, in its own
format, and applies the table with `apply_overrides`, which reports what it
could not apply:

```rust
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use tuikit::glyphs::{VettedGlyphs, marks};
use tuikit::theme::{OverrideValue, StyleSpec, Theme};

let mut theme = Theme::default();
theme.register("kind.folder", Color::Blue, "a folder's glyph");
theme.register("kind.link", StyleSpec::on(Color::Black, Color::Cyan), "a link's chip");
theme.register_alias("dim", "muted");

// The table the program read from its configuration file.
let table = [
    ("accent", OverrideValue::Text("magenta")),
    ("kind.folder", OverrideValue::Text("#3388ff")),
    ("no_such_key", OverrideValue::Text("red")),
];
let problems = theme.apply_overrides(table);
assert_eq!(problems.len(), 1, "the unknown key is reported, the rest applied");
assert_eq!(theme.accent, Color::Magenta);

// Drawing a role: a typed token directly, a named one by its name.
let folder = Span::styled("▸ docs", theme.style("kind.folder"));
let note = Span::styled("(empty)", Style::default().add_modifier(theme.muted));
assert_eq!(folder.style.fg, Some(Color::Rgb(0x33, 0x88, 0xff)));
assert!(note.style.add_modifier.contains(theme.muted));

// Glyphs: the shared marks are constants, and a glyph of the program's own
// vocabulary is measured and registered with a note on how.
let row = format!("{} docs", marks::POINTER);
let mut vetted = VettedGlyphs::default();
vetted.register("▸", "Geometric Shapes; DejaVu Sans Mono 1.00").expect("one cell");
assert!(vetted.unvetted(&format!("const A: &str = {:?};", row)).is_empty());
```

A glyph a terminal's font lacks is drawn from a fallback font and can spill
into the next cell, so every glyph the program draws should be in the vetted
table: the framework's marks are, and the program registers its own. The
program's tests then scan its source with `VettedGlyphs::unvetted` (the test
kit's `testkit::source::rust_files` walks a tree) and fail on a glyph nobody
measured. See [theme.md](controls/theme.md) and
[glyphs.md](controls/glyphs.md).

## Key tables

The framework's controls bring their keys with them: each resolves a standard
table from `tuikit::keys` (`MENU_KEYS`, `DIALOG_KEYS`, `READER_KEYS`,
`FORM_KEYS`, and the rest), and generates its footer and help from it. An
overlay of the program's own does the same with a table of its own:

1. An action enum, which implements `KeyAction` (what each action is for:
   abort, leave, apply, move, ...) and, for a help page, `HelpAction`.
2. A `const` table of rows over it: `shown` rows appear in the footer,
   `alias` rows bind a key without showing it, and `include` splices in a
   standard table instead of copying its rows. `Ctrl-C` comes first.
3. `resolve(table, key)` in `handle`, and an exhaustive `match` on the
   action, so a bound key the overlay ignores fails to compile.
4. The checks, in the program's tests: `mislabeled`, `unresolved`,
   `loose_characters`, `ctrl_c_first`, and, for a help page,
   `help::undescribed`.

```rust
use ratatui::crossterm::event::KeyCode;
use tuikit::help::{HelpAction, HelpEntry, undescribed};
use tuikit::keys::{
    self, Footer, KeyAction, KeyRole, KeyRow, alias, ch, ctrl, key, resolve, shown,
};
use tuikit::testkit::keys::{press, typed};

/// The keys of a color swatch the program draws itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Swatch {
    Abort,
    Next,
    Pick,
    Close,
}

impl KeyAction for Swatch {
    fn role(self) -> KeyRole {
        match self {
            Swatch::Abort => KeyRole::Abort,
            Swatch::Next => KeyRole::Move,
            Swatch::Pick => KeyRole::Apply,
            Swatch::Close => KeyRole::Leave,
        }
    }
}

impl HelpAction for Swatch {
    const LISTED: &'static [Self] = &[Swatch::Next, Swatch::Pick, Swatch::Close, Swatch::Abort];

    fn help(self) -> HelpEntry {
        match self {
            Swatch::Next => HelpEntry::new("Swatch", "Show the next color"),
            Swatch::Pick => HelpEntry::new("Swatch", "Use the color shown"),
            Swatch::Close => HelpEntry::new("Swatch", "Close without choosing"),
            Swatch::Abort => HelpEntry::new("Swatch", "Close everything open"),
        }
    }
}

const SWATCH_KEYS: &[KeyRow<Swatch>] = &[
    alias(ctrl('c'), Swatch::Abort),
    alias(ch('q'), Swatch::Abort),
    shown(key(KeyCode::Tab), Swatch::Next, "Tab", "next"),
    shown(key(KeyCode::Enter), Swatch::Pick, "Enter", "pick"),
    shown(key(KeyCode::Esc), Swatch::Close, "Esc", "close"),
];

// The footer is generated, the keys that leave last.
assert_eq!(Footer::leaving_last(SWATCH_KEYS).text(), "Tab next · Enter pick · Esc close");

// `handle` resolves and matches every action.
let meaning = |event| match resolve(SWATCH_KEYS, event) {
    Some(Swatch::Next) => "next",
    Some(Swatch::Pick) => "pick",
    Some(Swatch::Close | Swatch::Abort) => "close",
    None => "nothing",
};
assert_eq!(meaning(press(KeyCode::Tab)), "next");
assert_eq!(meaning(typed('q')), "close");
assert_eq!(meaning(tuikit::testkit::keys::ctrl('q')), "nothing", "a character binds as typed");

// The checks a program runs over every table it writes.
assert!(keys::mislabeled(SWATCH_KEYS).is_empty());
assert!(keys::unresolved(SWATCH_KEYS).is_empty());
assert!(keys::loose_characters(SWATCH_KEYS).is_empty());
assert!(keys::ctrl_c_first(SWATCH_KEYS));
assert!(undescribed(SWATCH_KEYS).is_empty());
```

Characters bind as typed: `ch('q')` fires on `q` with or without Shift and
never on `Ctrl-Q`; a chord is spelled out with `ctrl` or `alt`. The footer's
grammar is the key and a short verb with no "to" (house rule 8). See
[keys.md](controls/keys.md).

## The scoped keymap

The program's own keys, the ones no overlay owns, are one `Keymap`, as in the
file browser: `bind(id, scope, keys, action)` rows, each with a stable id, a
verb for its hint, help for its row on the help page, an optional rank for a
narrow status bar, and an optional availability predicate over the program's
context.

- **Scopes.** `Scope::Global` applies everywhere, `Scope::Pane(name)` while
  that pane has focus, and `Scope::Overlay(name)` while a mode of that name is
  on. `resolve(ActiveScopes::pane("files"), key)` looks in the most
  particular scope first, so one key can mean one thing in each pane.
- **No shadowing.** `build` refuses a keymap in which two keys of one scope
  can match one key event, so the table's order never decides what a key
  does. `overrides` lists where a pane's binding takes a global key, for a
  test to hold to the ones meant.
- **Availability.** A binding whose predicate says it cannot act still
  resolves, so the program can say why nothing happened (the file browser's
  `nothing to delete`); the status bar leaves it out, and a help page
  generated with the context dims it.
- **A control's table as a scope.** `KeymapBuilder::table(table, describe)`
  takes a pane control's standard table (the explorer's `EXPLORER_KEYS`) in
  as that pane's scope without copying its rows.

## Help and the status bar

A help page is generated, never written by hand, so it cannot disagree with
the keys:

- `help::keymap_sections(&keymap, &annotations)` makes the program's page
  from its keymap, one row per binding, with `HelpAnnotation`s for what a
  binding's one sentence cannot say; `help::unanchored` finds an annotation
  whose binding is gone.
- `help::sections_for(table)` makes an overlay's page from its key table and
  its `HelpAction` descriptions.
- `HelpPage::new(sections).pager(table)` is the page in a framed reader that
  scrolls it and closes on `table`'s keys; push it on the stack like any
  overlay.

The status bar names the keys that act now: the top overlay's own footer
(`Overlay::hints`) while one is open, and otherwise the program's keymap in
the active scopes (`KeyHintBar::keymap`), beside the program's own segments.
A message replaces the hints until the program clears it.

```rust
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::KeyCode;
use ratatui::layout::Rect;
use tuikit::feedback::Feedback;
use tuikit::help::{HelpAnnotation, HelpPage, HelpRow, keymap_sections, unanchored};
use tuikit::keys::{ActiveScopes, Bind, Keymap, KeyRow, PAGER_KEYS, PagerKey, Scope, bind, ch, include, key, shown};
use tuikit::priority_line::Segment;
use tuikit::status_bar::{Annotation, KeyHintBar, StatusBar};
use tuikit::testkit::buffer::rows;
use tuikit::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Do {
    Open,
    Help,
    Quit,
}

const KEYS: &[Bind<Do>] = &[
    bind("open", Scope::Pane("files"), &[key(KeyCode::Enter)], Do::Open)
        .verb("open")
        .help("Files", "Open the file under the cursor")
        .rank(5),
    bind("help", Scope::Global, &[ch('?')], Do::Help).verb("help").help("Program", "Show this help"),
    bind("quit", Scope::Global, &[ch('q')], Do::Quit).verb("quit").help("Program", "Quit").rank(9),
];

/// The help page's own keys: the standard reader's, and `?` to close it.
const HELP_KEYS: &[KeyRow<PagerKey>] = &[include(PAGER_KEYS), shown(ch('?'), PagerKey::Close, "?", "close")];

let keymap = Keymap::builder().rows(KEYS).build().expect("no key shadows another");
let theme = Theme::default();

// The help page, generated from the keymap, read in a pager of its own.
let notes = [HelpAnnotation::more("open", "; a folder opens in place")];
assert!(unanchored(&keymap, &notes).is_empty());
let mut help = HelpPage::<()>::new(keymap_sections(&keymap, &notes)).pager(HELP_KEYS);
let mut buf = Buffer::empty(Rect::new(0, 0, 70, 16));
help.draw(buf.area, &mut buf, &theme);
assert!(rows(&buf).iter().any(|row| row.contains("Open the file under the cursor; a folder opens")));

// The status bar: a segment of the program's, then the keys that act now,
// fitted to the width by rank.
let hints = KeyHintBar::new().keymap(&keymap, ActiveScopes::pane("files"), &(), 1, |_| Some(Annotation::new()));
let bar = StatusBar::new(&theme).segment(Segment::new("3 files").rank(4)).hints(&hints);
assert_eq!(bar.line(60).to_string(), "3 files · Enter open · ? help · q quit");
assert_eq!(bar.line(24).to_string(), "Enter open · q quit …");

// A message takes the bar's place.
let saved = Feedback::success("saved notes.txt");
assert_eq!(StatusBar::new(&theme).message(Some(&saved)).line(60).to_string(), "saved notes.txt");
```

See [help.md](controls/help.md) and [status_bar.md](controls/status_bar.md).

## Feedback: toasts, the log, and the clipboard

A report is a `feedback::Feedback` with a severity (info, success, warning,
error) and its text. The program decides which channel each report takes:
the status line (`Feedback::span`), a toast (`Toasts`, which go when their
time is up, errors last), or the message log (`Log<T>`, over a payload of the
program's own, read back in a `LogList`). Every channel styles by the
severity (house rule 11). Nothing reads the clock: the program passes the
moment a toast is shown and the moment to check against.

The framework writes nothing itself. `clipboard::copy` encodes the OSC 52
sequence that puts text on the terminal's clipboard and writes it to a sink
the program supplies, usually the writer its terminal draws to:

```rust
use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use tuikit::feedback::{Entry, Feedback, Log, Severity, Toasts, clipboard};
use tuikit::theme::Theme;

let theme = Theme::default();
let now = Instant::now();

// A toast, and the clock that retires it.
let mut toasts = Toasts::new();
toasts.show(Feedback::error("could not save notes.txt"), now);
let mut buf = Buffer::empty(Rect::new(0, 0, 60, 10));
toasts.render(buf.area, &mut buf, &theme);
toasts.tick(now + Duration::from_secs(1));
assert!(!toasts.is_empty(), "an error stays");
toasts.tick(now + Severity::Error.ttl());
assert!(toasts.is_empty());

// The log keeps what happened, with the program's payload.
let mut log: Log<&str> = Log::new(100);
log.push(Entry::new(1_000, Severity::Error, "save failed", "notes.txt"));
assert_eq!(log.unseen(), 1);

// The clipboard writes to the sink it is given: in a program, the terminal.
let mut terminal_output: Vec<u8> = Vec::new();
clipboard::copy(&mut terminal_output, "notes.txt").expect("the sink takes it");
assert!(terminal_output.starts_with(b"\x1b]52;c;"));
```

See [feedback.md](controls/feedback.md) and
[log_list.md](controls/log_list.md).

## Path fields and their source

A path field completes, checks, and expands what is typed, but reads the file
system only through a `path_field::PathSource` the program supplies: a
directory's entries, the kind of what is at a path, and the home directory.
The program's source is read-only (often a few lines over `std::fs`), and a
test supplies the test kit's `MemoryPaths` instead, so no test needs a real
directory. The field asks its source when its text changes and keeps the
answers, so drawing does no I/O (house rule 16).

```rust
use std::sync::Arc;

use ratatui::crossterm::event::KeyCode;
use tuikit::path_field::{PathField, PathKind, PathSource, Validity};
use tuikit::testkit::keys::{press, typed};
use tuikit::testkit::paths::MemoryPaths;

let disk = Arc::new(MemoryPaths::new().with_home("/home/me").dir("/home/me/notes").file("/home/me/todo.txt"));
let source: Arc<dyn PathSource + Send + Sync> = disk.clone();
let mut field = PathField::new("~/n", PathKind::Dir, source);

// `Tab` adds the rest of the best completion; a directory field offers
// directories only.
assert!(field.handle(press(KeyCode::Tab)).consumed);
assert_eq!(field.value(), "~/notes");
assert_eq!(field.validity(), Validity::Exists);

// The committed value has `~` expanded; drawing asks the source nothing.
let asked = disk.calls();
let _ = field.view().focused(true).line(40, &tuikit::theme::Theme::default());
assert_eq!(disk.calls(), asked);
assert_eq!(field.expanded_value(), "/home/me/notes");
field.handle(typed('/'));
```

A form takes a path field as one of its rows (`form::Field::path`), and its
`on_submit` reads the expanded value from the form's values. See
[path_field.md](controls/path_field.md) and [form.md](controls/form.md).

## Testing with the test kit

With the `testkit` feature on in its dev-dependencies, a program tests its
screens the way a user drives them, without a terminal:

- **Key scripts** (`testkit::keys`): `script("jj<Enter>")` or
  `parse("j<Tab><C-r>")` writes a run of presses; `press`, `typed`, and
  `ctrl` build one. No test writes a `KeyEvent` literal.
- **The frame driver** (`testkit::render::FrameDriver`): draws a frame off
  screen through the program's own draw callback, and `drive` presses keys
  with a frame drawn before each, as a live loop does, so a page key moves by
  the rows the last frame showed.
- **Buffer probes** (`testkit::buffer`): a frame's text, its rows, the style
  under a piece of text, and a modal's frame and the text inside it.
- **Goldens** (`testkit::golden`): a frame written as text that keeps every
  color and modifier, so a test fails when only a color changes.
  `assert_golden(path, buffer, std::fs::write)` checks a frame against a file;
  running the test with `TUIKIT_ACCEPT_GOLDENS=1` hands the new frame to the
  writer instead, so a deliberate change rewrites its golden for review.
- **The conformance suite** (`testkit::conformance`): the program lists its
  modals in a `Registry` (how to open each, its key tables, its footer) with
  a `Rig` that draws its screen and routes its keys, and `assert_conventions`
  holds every modal to the shared rules: drawn, contained in its frame, one
  title look, a cursor that moves no text, no word cut at the edge, keys by
  role, the generated footer in its look and place, and the close keys.

```rust
use ratatui::crossterm::event::KeyEvent;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use tuikit::selection::{self, Cursor, Ends};
use tuikit::testkit::{buffer, golden, keys, render::FrameDriver};
use tuikit::theme::Theme;

/// A list the test drives: `j` moves its cursor, and it draws a row each.
struct List {
    rows: Vec<&'static str>,
    cursor: usize,
}

let theme = Theme::default();
let mut list = List { rows: vec!["alpha", "beta", "gamma"], cursor: 0 };
let mut driver = FrameDriver::new(12, 3);
let draw = |list: &mut List, frame: &mut ratatui::Frame<'_>| {
    let lines: Vec<Line> = (list.rows.iter().enumerate())
        .map(|(at, row)| selection::row([(*row).into()], Cursor::of(at == list.cursor, true), &theme))
        .collect();
    frame.render_widget(Paragraph::new(lines), frame.area());
};
driver.drive(&mut list, keys::script("jj"), draw, |list, _: KeyEvent| {
    list.cursor = Ends::Clamp.step(list.cursor, list.rows.len(), 1);
});
let shown = driver.draw(|frame| draw(&mut list, frame));
assert_eq!(buffer::rows(shown)[2], "► gamma     ");

// The golden keeps the highlight: the same text with another cursor look
// would not match it.
let styled = golden::styled(shown);
assert!(styled.starts_with("frame 12x3"));
assert!(golden::compare(&styled, &styled).is_ok());
```

A program's conformance test looks like the one on
[testkit.md](controls/testkit.md); it registers every modal the program has,
and fails a test of its own when a modal is missing from the registry.

## Staying independent

The framework knows nothing about any program that hosts it, and guard tests
keep it that way: its manifest names no host crate and only allowlisted
dependencies, no file in the crate uses the host's domain vocabulary, and its
source starts no process and writes no file. A program needs to do nothing
for this, but code it would like to move into the framework has to pass the
same guards: anything that needs a concrete type, a word, or an effect of the
program's stays in the program as an adapter, and the framework takes only
what survives being generic over `M`, `Cx`, `Id`, and `R`. The rules, how
the guards match, and how to ask for a reviewed exception are in
[contributing.md](contributing.md#independence-guards).

A program can hold its own code to the framework's rules with the same
mechanisms: a scan for raw colors outside the file where it registers its
named styles, the glyph scan above, and the conformance suite.

## Where to go next

- [architecture.md](architecture.md): the layers, the module map, and what
  the framework never does.
- The reference page of each module, under [controls/](controls/_template.md).
- [house-rules.md](house-rules.md): how every control behaves, and why.
- [contributing.md](contributing.md): adding a control to the framework.
- [roadmap.md](roadmap.md): what may come later, and what is left out.
